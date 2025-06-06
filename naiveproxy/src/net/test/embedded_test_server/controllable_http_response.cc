// Copyright 2017 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "net/test/embedded_test_server/controllable_http_response.h"

#include "base/check_op.h"
#include "base/functional/bind.h"
#include "base/strings/stringprintf.h"
#include "base/task/single_thread_task_runner.h"
#include "net/base/tracing.h"
#include "net/test/embedded_test_server/http_response.h"

namespace net::test_server {
class Interceptor : public HttpResponse {
 public:
  using ResponceCallback =
      base::OnceCallback<void(scoped_refptr<base::SingleThreadTaskRunner>,
                              base::WeakPtr<HttpResponseDelegate>)>;

  explicit Interceptor(
      scoped_refptr<base::SingleThreadTaskRunner> controller_task_runner,
      ResponceCallback callback)
      : controller_task_runner_(controller_task_runner),
        callback_(std::move(callback)) {}

  Interceptor(const Interceptor&) = delete;
  Interceptor& operator=(const Interceptor&) = delete;

  ~Interceptor() override = default;

 private:
  void SendResponse(base::WeakPtr<HttpResponseDelegate> delegate) override {
    scoped_refptr<base::SingleThreadTaskRunner> task_runner =
        base::SingleThreadTaskRunner::GetCurrentDefault();
    CHECK(task_runner);
    controller_task_runner_->PostTask(
        FROM_HERE, base::BindOnce(&Interceptor::OnRequest, std::move(callback_),
                                  std::move(task_runner), std::move(delegate)));
  }

  static void OnRequest(ResponceCallback callback,
                        scoped_refptr<base::SingleThreadTaskRunner>
                            embedded_test_server_task_runner,
                        base::WeakPtr<HttpResponseDelegate> delegate) {
    std::move(callback).Run(std::move(embedded_test_server_task_runner),
                            std::move(delegate));
  }

  scoped_refptr<base::SingleThreadTaskRunner> controller_task_runner_;
  ResponceCallback callback_;
};

bool DoesRequestMatchURL(const HttpRequest& request,
                         const std::string& relative_url,
                         bool relative_url_is_prefix) {
  return request.relative_url == relative_url ||
         (relative_url_is_prefix &&
          request.relative_url.starts_with(relative_url));
}

ControllableHttpResponse::ControllableHttpResponse(
    EmbeddedTestServer* embedded_test_server,
    const std::string& relative_url,
    bool relative_url_is_prefix) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  embedded_test_server->RegisterRequestHandler(base::BindRepeating(
      RequestHandler, weak_ptr_factory_.GetWeakPtr(),
      base::SingleThreadTaskRunner::GetCurrentDefault(),
      base::Owned(new bool(true)), relative_url, relative_url_is_prefix));
}

ControllableHttpResponse::ControllableHttpResponse(
    scoped_refptr<base::SingleThreadTaskRunner>
        embedded_test_server_task_runner,
    base::WeakPtr<HttpResponseDelegate> delegate,
    std::unique_ptr<HttpRequest> http_request)
    : state_(State::READY_TO_SEND_DATA),
      embedded_test_server_task_runner_(embedded_test_server_task_runner),
      delegate_(delegate),
      http_request_(std::move(http_request)) {}

ControllableHttpResponse::~ControllableHttpResponse() = default;

void ControllableHttpResponse::WaitForRequest() {
  TRACE_EVENT("test", "ControllableHttpResponse::WaitForRequest");
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  CHECK_EQ(State::WAITING_FOR_REQUEST, state_)
      << "WaitForRequest() called twice.";
  loop_.Run();
  CHECK(embedded_test_server_task_runner_);
  state_ = State::READY_TO_SEND_DATA;
}

void ControllableHttpResponse::Send(
    net::HttpStatusCode http_status,
    const std::string& content_type,
    const std::string& content,
    const std::vector<std::string>& cookies,
    const std::vector<std::string>& extra_headers) {
  TRACE_EVENT("test", "ControllableHttpResponse::Send", "http_status",
              http_status, "content_type", content_type, "content", content,
              "cookies", cookies);
  std::string content_data(base::StringPrintf(
      "HTTP/1.1 %d %s\nContent-type: %s\n", static_cast<int>(http_status),
      net::GetHttpReasonPhrase(http_status), content_type.c_str()));
  for (auto& cookie : cookies)
    content_data += "Set-Cookie: " + cookie + "\n";
  for (auto& header : extra_headers)
    content_data += header + "\n";
  content_data += "\n";
  content_data += content;
  Send(content_data);
}

void ControllableHttpResponse::Send(const std::string& bytes) {
  TRACE_EVENT("test", "ControllableHttpResponse::Send", "bytes", bytes);
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  CHECK_EQ(State::READY_TO_SEND_DATA, state_) << "Send() called without any "
                                                 "opened connection. Did you "
                                                 "call WaitForRequest()?";
  base::RunLoop loop;
  embedded_test_server_task_runner_->PostTask(
      FROM_HERE, base::BindOnce(&HttpResponseDelegate::SendContents, delegate_,
                                bytes, loop.QuitClosure()));
  loop.Run();
}

void ControllableHttpResponse::Done() {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  CHECK_EQ(State::READY_TO_SEND_DATA, state_) << "Done() called without any "
                                                 "opened connection. Did you "
                                                 "call WaitForRequest()?";
  embedded_test_server_task_runner_->PostTask(
      FROM_HERE,
      base::BindOnce(&HttpResponseDelegate::FinishResponse, delegate_));
  state_ = State::DONE;
}

bool ControllableHttpResponse::has_received_request() {
  return loop_.AnyQuitCalled();
}

void ControllableHttpResponse::OnRequest(
    std::unique_ptr<HttpRequest> http_request,
    scoped_refptr<base::SingleThreadTaskRunner>
        embedded_test_server_task_runner,
    base::WeakPtr<HttpResponseDelegate> delegate) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  CHECK(embedded_test_server_task_runner);
  CHECK(!embedded_test_server_task_runner_)
      << "A ControllableHttpResponse can only handle one request at a time";
  embedded_test_server_task_runner_ = embedded_test_server_task_runner;
  delegate_ = delegate;
  http_request_ = std::move(http_request);
  loop_.Quit();
}

// Helper function used in the ControllableHttpResponse constructor.
// static
std::unique_ptr<HttpResponse> ControllableHttpResponse::RequestHandler(
    base::WeakPtr<ControllableHttpResponse> controller,
    scoped_refptr<base::SingleThreadTaskRunner> controller_task_runner,
    bool* available,
    const std::string& relative_url,
    bool relative_url_is_prefix,
    const HttpRequest& request) {
  if (!*available)
    return nullptr;

  if (DoesRequestMatchURL(request, relative_url, relative_url_is_prefix)) {
    *available = false;
    return std::make_unique<Interceptor>(
        std::move(controller_task_runner),
        base::BindOnce(&ControllableHttpResponse::OnRequest,
                       std::move(controller),
                       std::make_unique<HttpRequest>(request)));
  }

  return nullptr;
}

ControllableHttpResponseManager::ControllableHttpResponseManager(
    EmbeddedTestServer* embedded_test_server,
    const std::string& relative_url,
    bool relative_url_is_prefix) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  embedded_test_server->RegisterRequestHandler(
      base::BindRepeating(RequestHandler, weak_ptr_factory_.GetWeakPtr(),
                          base::SingleThreadTaskRunner::GetCurrentDefault(),
                          relative_url, relative_url_is_prefix));
}

ControllableHttpResponseManager::~ControllableHttpResponseManager() = default;

std::unique_ptr<HttpResponse> ControllableHttpResponseManager::RequestHandler(
    base::WeakPtr<ControllableHttpResponseManager> controller,
    scoped_refptr<base::SingleThreadTaskRunner> controller_task_runner,
    const std::string& relative_url,
    bool relative_url_is_prefix,
    const HttpRequest& request) {
  if (DoesRequestMatchURL(request, relative_url, relative_url_is_prefix)) {
    return std::make_unique<Interceptor>(
        std::move(controller_task_runner),
        base::BindOnce(&ControllableHttpResponseManager::OnRequest,
                       std::move(controller),
                       std::make_unique<HttpRequest>(request)));
  }

  return nullptr;
}

void ControllableHttpResponseManager::OnRequest(
    std::unique_ptr<HttpRequest> http_request,
    scoped_refptr<base::SingleThreadTaskRunner>
        embedded_test_server_task_runner,
    base::WeakPtr<HttpResponseDelegate> delegate) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  CHECK(!current_response_) << "A ControllableHttpResponseManager can only "
                               "handle one request at a time";
  current_response_ =
      std::unique_ptr<ControllableHttpResponse>(new ControllableHttpResponse(
          embedded_test_server_task_runner, delegate, std::move(http_request)));
  if (loop_) {
    loop_->Quit();
  }
}

std::unique_ptr<ControllableHttpResponse>
ControllableHttpResponseManager::WaitForRequest() {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  if (current_response_) {
    return std::move(current_response_);
  }

  loop_ = std::make_unique<base::RunLoop>();
  loop_->Run();
  return std::move(current_response_);
}

}  // namespace net::test_server

/*
 * httpproxy.h - the header file of HttpProxy class
 *
 * This class enables transparent HTTP proxy that handles data transfer
 * and send/recv them via upper-level SOCKS5 proxy
 *
 * Copyright (C) 2015-2016 Symeon Huang <hzwhuang@gmail.com>
 *
 * This file is part of the libQtShadowsocks.
 *
 * libQtShadowsocks is free software; you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as published
 * by the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 *
 * libQtShadowsocks is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with libQtShadowsocks; see the file LICENSE. If not, see
 * <http://www.gnu.org/licenses/>.
 */

#pragma once

#include "SocketStream.hpp"

#include <QDebug>
#include <QNetworkProxy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QUrl>

#ifndef FD_SETSIZE
#define FD_SETSIZE 1024
#endif

namespace Qv2rayPlugin
{
    namespace Utils
    {
        class HttpProxy : public QTcpServer
        {
            Q_OBJECT
          public:
            HttpProxy() : QTcpServer()
            {
                this->setMaxPendingConnections(FD_SETSIZE);
            }

            HttpProxy(const HttpProxy &) = delete;

            /*
             * DO NOT use listen() function, use httpListen instead
             * The socks_port is local socks proxy server port
             */
            bool httpListen(const QHostAddress &http_addr, uint16_t http_port, uint16_t socks_port)
            {
                bool isAny = http_addr == QHostAddress::AnyIPv4 || http_addr == QHostAddress::AnyIPv6;
                upstreamProxy = QNetworkProxy(QNetworkProxy::Socks5Proxy, isAny ? "127.0.0.1" : http_addr.toString(), socks_port);
                return this->listen(http_addr, http_port);
            }

          protected:
            void incomingConnection(qintptr socketDescriptor)
            {
                QTcpSocket *socket = new QTcpSocket(this);
                connect(socket, &QTcpSocket::readyRead, this, &HttpProxy::onSocketReadyRead);
                connect(socket, &QTcpSocket::disconnected, socket, &QTcpSocket::deleteLater);
                connect(socket, &QAbstractSocket::errorOccurred, this, &HttpProxy::onSocketError);
                socket->setSocketDescriptor(socketDescriptor);
            }

          private:
            QNetworkProxy upstreamProxy;

          private slots:
            void onSocketError(QAbstractSocket::SocketError err)
            {
                if (err != QAbstractSocket::RemoteHostClosedError)
                {
                    QDebug(QtMsgType::QtWarningMsg) << "HTTP socket error: " << err;
                }
                sender()->deleteLater();
            }

            void onSocketReadyRead()
            {
                QTcpSocket *socket = qobject_cast<QTcpSocket *>(sender());
                QTcpSocket *proxySocket = nullptr;

                QByteArray reqData = socket->readAll();
                int pos = reqData.indexOf("\r\n");
                QByteArray reqLine = reqData.left(pos);
                reqData.remove(0, pos + 2);

                QList<QByteArray> entries = reqLine.split(' ');
                QByteArray method = entries.value(0);
                QByteArray address = entries.value(1);
                QByteArray version = entries.value(2);

                QString host;
                uint16_t port;
                QString key;

                if (method != "CONNECT")
                {
                    QUrl url = QUrl::fromEncoded(address);
                    if (!url.isValid())
                    {
                        QDebug(QtMsgType::QtCriticalMsg) << "Invalid URL: " << url;
                        socket->disconnectFromHost();
                        return;
                    }
                    host = url.host();
                    port = url.port(80);
                    QString req = url.path();
                    if (url.hasQuery())
                    {
                        req.append('?').append(url.query());
                    }
                    reqLine = method + " " + req.toUtf8() + " " + version + "\r\n";
                    reqData.prepend(reqLine);
                    key = host + ':' + QString::number(port);
                    proxySocket = socket->findChild<QTcpSocket *>(key);
                    if (proxySocket)
                    {
                        proxySocket->write(reqData);
                        return; // if we find an existing socket, then use it and return
                    }
                }
                else
                {
                    // CONNECT method
                    /*
                     * http://tools.ietf.org/html/draft-luotonen-ssl-tunneling-03
                     * the first line would CONNECT HOST:PORT VERSION
                     */
                    QList<QByteArray> host_port_list = address.split(':');
                    host = QString(host_port_list.first());
                    port = host_port_list.last().toUShort();
                }

                proxySocket = new QTcpSocket(socket);
                proxySocket->setProxy(upstreamProxy);
                if (method != "CONNECT")
                {
                    proxySocket->setObjectName(key);
                    proxySocket->setProperty("reqData", reqData);
                    connect(proxySocket, &QTcpSocket::connected, this, &HttpProxy::onProxySocketConnected);
                    connect(proxySocket, &QTcpSocket::readyRead, this, &HttpProxy::onProxySocketReadyRead);
                }
                else
                {
                    connect(proxySocket, &QTcpSocket::connected, this, &HttpProxy::onProxySocketConnectedHttps);
                }
                connect(proxySocket, &QTcpSocket::disconnected, proxySocket, &QTcpSocket::deleteLater);
                connect(socket, &QAbstractSocket::errorOccurred, this, &HttpProxy::onSocketError);
                proxySocket->connectToHost(host, port);
            }

            void onProxySocketConnected()
            {
                QTcpSocket *proxySocket = qobject_cast<QTcpSocket *>(sender());
                QByteArray reqData = proxySocket->property("reqData").toByteArray();
                proxySocket->write(reqData);
            }

            // this function is used for HTTPS transparent proxy
            void onProxySocketConnectedHttps()
            {
                QTcpSocket *proxySocket = qobject_cast<QTcpSocket *>(sender());
                QTcpSocket *socket = qobject_cast<QTcpSocket *>(proxySocket->parent());
                disconnect(socket, &QTcpSocket::readyRead, this, &HttpProxy::onSocketReadyRead);

                /*
                 * once it's connected
                 * we use a light-weight SocketStream class to do the job
                 */
                auto stream = new SocketStream(socket, proxySocket, this);
                connect(socket, &QTcpSocket::disconnected, stream, &SocketStream::deleteLater);
                connect(proxySocket, &QTcpSocket::disconnected, stream, &SocketStream::deleteLater);
                static const auto httpsHeader = "HTTP/1.0 200 Connection established\r\n\r\n";
                socket->write(httpsHeader);
            }
            void onProxySocketReadyRead()
            {
                QTcpSocket *proxySocket = qobject_cast<QTcpSocket *>(sender());
                QTcpSocket *socket = qobject_cast<QTcpSocket *>(proxySocket->parent());
                socket->write(proxySocket->readAll());
            }
        };
    } // namespace Utils
} // namespace Qv2rayPlugin

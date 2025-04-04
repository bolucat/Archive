// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2021-2024 Chilledheart  */

#include "gtk/option_dialog.hpp"

#include <absl/flags/flag.h>
#include <glib/gi18n.h>

#include "config/config.hpp"
#include "core/logging.hpp"
#include "core/utils.hpp"
#include "gtk/utils.hpp"
#include "net/network.hpp"

OptionDialog::OptionDialog(const std::string& title, GtkWindow* parent, bool modal)
    : impl_(GTK_DIALOG(gtk_dialog_new_with_buttons(title.c_str(),
                                                   parent,
                                                   (modal ? GTK_DIALOG_MODAL : GTK_DIALOG_DESTROY_WITH_PARENT),
                                                   nullptr,
                                                   nullptr))) {
  gtk_window_set_position(GTK_WINDOW(impl_), GTK_WIN_POS_CENTER);

  GtkGrid* grid = GTK_GRID(gtk_grid_new());

  auto tcp_keep_alive_label = gtk_label_new(_("TCP keep alive"));
  auto tcp_keep_alive_cnt_label = gtk_label_new(_("The number of TCP keep-alive probes"));
  auto tcp_keep_alive_idle_timeout_label = gtk_label_new(_("TCP keep alive after idle"));
  auto tcp_keep_alive_interval_label = gtk_label_new(_("TCP keep alive interval"));

  auto enable_post_quantum_kyber_label = gtk_label_new(_("Enables post-quantum key-agreements in TLS 1.3 connections"));
  auto tcp_congestion_algorithm = gtk_label_new(_("TCP Congestion Algorithm"));

  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_label), 0, 0, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_cnt_label), 0, 1, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_idle_timeout_label), 0, 2, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_interval_label), 0, 3, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(enable_post_quantum_kyber_label), 0, 4, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_congestion_algorithm), 0, 5, 1, 1);

  tcp_keep_alive_ = GTK_CHECK_BUTTON(gtk_check_button_new());
  tcp_keep_alive_cnt_ = GTK_ENTRY(gtk_entry_new());
  tcp_keep_alive_idle_timeout_ = GTK_ENTRY(gtk_entry_new());
  tcp_keep_alive_interval_ = GTK_ENTRY(gtk_entry_new());
  enable_post_quantum_kyber_ = GTK_CHECK_BUTTON(gtk_check_button_new());

  algorithms_ = net::GetTCPAvailableCongestionAlgorithms();

  tcp_congestion_algorithm_ = GTK_COMBO_BOX_TEXT(gtk_combo_box_text_new());
  for (const auto& algorithm : algorithms_) {
    gtk_combo_box_text_append_text(tcp_congestion_algorithm_, algorithm.c_str());
  }

  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_), 1, 0, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_cnt_), 1, 1, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_idle_timeout_), 1, 2, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_keep_alive_interval_), 1, 3, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(enable_post_quantum_kyber_), 1, 4, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(tcp_congestion_algorithm_), 1, 5, 1, 1);

  gtk_widget_set_margin_top(GTK_WIDGET(grid), 12);
  gtk_widget_set_margin_bottom(GTK_WIDGET(grid), 12);

#if GTK_CHECK_VERSION(3, 12, 0)
  gtk_widget_set_margin_start(GTK_WIDGET(grid), 12);
  gtk_widget_set_margin_end(GTK_WIDGET(grid), 12);
#else
  gtk_widget_set_margin_left(GTK_WIDGET(grid), 12);
  gtk_widget_set_margin_right(GTK_WIDGET(grid), 12);
#endif
  gtk_grid_set_column_spacing(GTK_GRID(grid), 12);
  gtk_grid_set_row_spacing(GTK_GRID(grid), 6);

  okay_button_ = GTK_BUTTON(gtk_button_new());
  gtk_button_set_label(okay_button_, _("Okay"));

  cancel_button_ = GTK_BUTTON(gtk_button_new());
  gtk_button_set_label(cancel_button_, _("Cancel"));

  auto okay_callback = [](GtkButton* self, gpointer pointer) {
    OptionDialog* window = (OptionDialog*)pointer;
    window->OnOkayButtonClicked();
  };

  g_signal_connect(G_OBJECT(okay_button_), "clicked", G_CALLBACK(*okay_callback), this);

  auto cancel_callback = [](GtkButton* self, gpointer pointer) {
    OptionDialog* window = (OptionDialog*)pointer;
    window->OnCancelButtonClicked();
  };

  g_signal_connect(G_OBJECT(cancel_button_), "clicked", G_CALLBACK(*cancel_callback), this);

  gtk_grid_attach(grid, GTK_WIDGET(okay_button_), 0, 6, 1, 1);
  gtk_grid_attach(grid, GTK_WIDGET(cancel_button_), 1, 6, 1, 1);

  gtk_container_add(GTK_CONTAINER(gtk_dialog_get_content_area(impl_)), GTK_WIDGET(grid));

  LoadChanges();

  gtk_widget_show_all(GTK_WIDGET(gtk_dialog_get_content_area(impl_)));
}

OptionDialog::~OptionDialog() {
  gtk_widget_destroy(GTK_WIDGET(impl_));
}

void OptionDialog::OnOkayButtonClicked() {
  if (!OnSave()) {
    return;
  }
  config::SaveConfig();
  gtk_dialog_response(impl_, GTK_RESPONSE_ACCEPT);
}

void OptionDialog::OnCancelButtonClicked() {
  gtk_dialog_response(impl_, GTK_RESPONSE_CANCEL);
}

gint OptionDialog::run() {
  return gtk_dialog_run(impl_);
}

void OptionDialog::LoadChanges() {
  gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(tcp_keep_alive_), absl::GetFlag(FLAGS_tcp_keep_alive));
  auto tcp_keep_alive_cnt_str = std::to_string(absl::GetFlag(FLAGS_tcp_keep_alive_cnt));
  gtk_entry_set_text(tcp_keep_alive_cnt_, tcp_keep_alive_cnt_str.c_str());
  auto tcp_keep_alive_idle_timeout_str = std::to_string(absl::GetFlag(FLAGS_tcp_keep_alive_idle_timeout));
  gtk_entry_set_text(tcp_keep_alive_idle_timeout_, tcp_keep_alive_idle_timeout_str.c_str());
  auto tcp_keep_alive_interval_str = std::to_string(absl::GetFlag(FLAGS_tcp_keep_alive_interval));
  gtk_entry_set_text(tcp_keep_alive_interval_, tcp_keep_alive_interval_str.c_str());

  gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(enable_post_quantum_kyber_),
                               absl::GetFlag(FLAGS_enable_post_quantum_kyber));

  auto algorithm = absl::GetFlag(FLAGS_tcp_congestion_algorithm);
  unsigned int i;
  for (i = 0; i < std::size(algorithms_); ++i) {
    if (algorithm == algorithms_[i])
      break;
  }

  // first is unset
  if (i == std::size(algorithms_)) {
    i = 0;
  }
  gtk_combo_box_set_active(GTK_COMBO_BOX(tcp_congestion_algorithm_), i);
}

bool OptionDialog::OnSave() {
  auto tcp_keep_alive = gtk_toggle_button_get_active(GTK_TOGGLE_BUTTON(tcp_keep_alive_));

  int tcp_keep_alive_cnt;
  if (!StringToInt(gtk_entry_get_text(tcp_keep_alive_cnt_), &tcp_keep_alive_cnt) || tcp_keep_alive_cnt < 0) {
    LOG(WARNING) << "invalid options: tcp_keep_alive_cnt";
    return false;
  }

  int tcp_keep_alive_idle_timeout;
  if (!StringToInt(gtk_entry_get_text(tcp_keep_alive_idle_timeout_), &tcp_keep_alive_idle_timeout) ||
      tcp_keep_alive_idle_timeout < 0) {
    LOG(WARNING) << "invalid options: tcp_keep_alive_idle_timeout";
    return false;
  }

  int tcp_keep_alive_interval;
  if (!StringToInt(gtk_entry_get_text(tcp_keep_alive_interval_), &tcp_keep_alive_interval) ||
      tcp_keep_alive_interval < 0) {
    LOG(WARNING) << "invalid options: tcp_keep_alive_interval";
    return false;
  }

  auto enable_post_quantum_kyber = gtk_toggle_button_get_active(GTK_TOGGLE_BUTTON(enable_post_quantum_kyber_));

  absl::SetFlag(&FLAGS_tcp_keep_alive, tcp_keep_alive);
  absl::SetFlag(&FLAGS_tcp_keep_alive_cnt, tcp_keep_alive_cnt);
  absl::SetFlag(&FLAGS_tcp_keep_alive_idle_timeout, tcp_keep_alive_idle_timeout);
  absl::SetFlag(&FLAGS_tcp_keep_alive_interval, tcp_keep_alive_interval);

  absl::SetFlag(&FLAGS_enable_post_quantum_kyber, enable_post_quantum_kyber);

  gchar* algorithm_cstr = gtk_combo_box_text_get_active_text(tcp_congestion_algorithm_);
  if (algorithm_cstr == nullptr || std::string_view(algorithm_cstr).empty()) {
    absl::SetFlag(&FLAGS_tcp_congestion_algorithm, std::string());
  } else {
    absl::SetFlag(&FLAGS_tcp_congestion_algorithm, algorithm_cstr);
  }

  return true;
}

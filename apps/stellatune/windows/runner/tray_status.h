#ifndef RUNNER_TRAY_STATUS_H_
#define RUNNER_TRAY_STATUS_H_

#include <flutter/binary_messenger.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>
#include <windows.h>
#include <shellapi.h>

// tray_manager 0.5.x doesn't propagate Shell_NotifyIcon errors. Its icon uses
// the root window and uID 1; check shell registration before hiding the window.
inline void RegisterTrayStatusChannel(flutter::BinaryMessenger* messenger,
                                      HWND window) {
  flutter::MethodChannel<flutter::EncodableValue> channel(
      messenger, "stellatune/tray_status",
      &flutter::StandardMethodCodec::GetInstance());
  channel.SetMethodCallHandler(
      [window](const auto& call, auto result) {
        if (call.method_name() != "isAvailable") {
          result->NotImplemented();
          return;
        }
        NOTIFYICONIDENTIFIER icon = {};
        icon.cbSize = sizeof(icon);
        icon.hWnd = window;
        icon.uID = 1;
        RECT bounds = {};
        result->Success(flutter::EncodableValue(
            SUCCEEDED(Shell_NotifyIconGetRect(&icon, &bounds))));
      });
}

#endif  // RUNNER_TRAY_STATUS_H_

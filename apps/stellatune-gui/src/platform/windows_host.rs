#![allow(dead_code)]

#![cfg(target_os = "windows")]

use std::num::NonZeroIsize;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use raw_window_handle::{
    HandleError, HasWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowHandle, WindowsDisplayHandle,
};
use tracing::{error, info};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, EndPaint, FillRgn, FrameRgn,
    InvalidateRect, PAINTSTRUCT, SetBkMode, SetTextColor, SetWindowRgn, TextOutW, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryA, GetProcAddress};
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GWL_EXSTYLE, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW,
    GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT,
    HTTOP, HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, IsZoomed, LoadCursorW, MSG, PM_REMOVE, PeekMessageW,
    PostQuitMessage, RegisterClassW, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SendMessageW,
    SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
    WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCCREATE,
    WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_PAINT, WM_QUIT, WM_SIZE, WNDCLASSW, LWA_ALPHA,
    WS_EX_LAYERED, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_THICKFRAME, WS_VISIBLE,
};
use winit::dpi::PhysicalSize;

use crate::app::GuiApp;
use crate::platform::host::{SurfaceHandles, WindowHost};
use crate::platform::input::{InputAction, map_windows_virtual_key};
use crate::platform::windows::composition_presenter::CompositionPresenterRuntime;
use crate::runtime::RuntimeServices;

const WINDOW_CLASS_NAME: &str = "StellatuneGuiNativeWindow";
const WINDOW_TITLE: &str = "Stellatune GUI";
const WINDOW_WIDTH: i32 = 1360;
const WINDOW_HEIGHT: i32 = 860;
const RENDER_UI: bool = true;
const WINDOW_CORNER_RADIUS: i32 = 15;
const USE_NATIVE_WINDOW_REGION: bool = false;
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
const WCA_ACCENT_POLICY: u32 = 19;
const RESIZE_BORDER_THICKNESS: i32 = 8;

#[derive(Debug, Clone, Copy)]
enum BackdropMode {
    LayeredAlpha,
    SwcaAcrylic,
    SwcaBlur,
    None,
}

pub fn run(runtime: RuntimeServices) -> Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(null());
        if hinstance.is_null() {
            return Err(anyhow!("GetModuleHandleW failed"));
        }

        let class_name = wide(WINDOW_CLASS_NAME);
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: hinstance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wnd_class);

        let host = Arc::new(WindowsWindowHost::new(hinstance as isize));
        let mut shell = Box::new(WindowsShell::new(runtime, Arc::clone(&host)));
        let shell_ptr = shell.as_mut() as *mut WindowsShell;

        let title = wide(WINDOW_TITLE);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_VISIBLE | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_THICKFRAME,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            null_mut(),
            null_mut(),
            hinstance,
            shell_ptr.cast(),
        );
        if hwnd.is_null() {
            return Err(anyhow!("CreateWindowExW failed"));
        }

        host.attach(hwnd);
        host.update_window_region();
        shell.attach_composition_runtime(crate::platform::windows::host::bootstrap_composition_runtime(
            hwnd as isize,
        ));
        host.apply_backdrop();
        if RENDER_UI {
            shell.initialize()?;
        }

        ShowWindow(hwnd, SW_SHOW);
        host.request_redraw();
        UpdateWindow(hwnd);

        info!("stellatune-gui native windows host initialized");

        let result = shell.run_loop();
        drop(shell);
        result
    }
}

struct WindowsShell {
    runtime: RuntimeServices,
    host: Arc<WindowsWindowHost>,
    composition_runtime: Option<CompositionPresenterRuntime>,
    app: Option<GuiApp>,
    last_frame_at: Instant,
    exit_requested: bool,
}

impl WindowsShell {
    fn new(runtime: RuntimeServices, host: Arc<WindowsWindowHost>) -> Self {
        Self {
            runtime,
            host,
            composition_runtime: None,
            app: None,
            last_frame_at: Instant::now(),
            exit_requested: false,
        }
    }

    fn attach_composition_runtime(&mut self, runtime: Option<CompositionPresenterRuntime>) {
        self.composition_runtime = runtime;
    }

    fn initialize(&mut self) -> Result<()> {
        let app = GuiApp::new(self.host.clone(), self.runtime.clone(), self.host.size())?;
        app.request_redraw();
        self.app = Some(app);
        Ok(())
    }

    fn run_loop(&mut self) -> Result<()> {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            loop {
                while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_QUIT {
                        return Ok(());
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                self.pump_runtime();

                if self.exit_requested || self.app.as_ref().is_some_and(|app| app.close_requested()) {
                    DestroyWindow(self.host.hwnd());
                    self.exit_requested = false;
                }

                if self.app.as_ref().is_some_and(|app| {
                    app.needs_continuous_redraw() || self.last_frame_at.elapsed() > Duration::from_millis(250)
                }) {
                    self.host.request_redraw();
                }

                std::thread::sleep(Duration::from_millis(8));
            }
        }
    }

    fn pump_runtime(&mut self) {
        let Some(app) = self.app.as_mut() else {
            return;
        };

        while let Ok(event) = app.try_recv_runtime_event() {
            app.handle_runtime_event(event);
        }
    }

    fn render_current_frame(&mut self) {
        if !RENDER_UI {
            return;
        }

        let Some(app) = self.app.as_mut() else {
            return;
        };

        if let Some(runtime) = self.composition_runtime.as_mut() {
            let effect_frame = app.composition_effect_frame();
            let ui_frame = match app.composition_ui_frame() {
                Ok(frame) => frame,
                Err(error) => {
                    error!(error = %error, "composition ui frame build failed");
                    self.exit_requested = true;
                    return;
                },
            };
            if let Err(error) = runtime.present_composed_frame(self.host.size(), &effect_frame, &ui_frame) {
                error!(error = %error, "composition composed frame present failed");
                self.exit_requested = true;
                return;
            }
            app.frame_presented();
            self.last_frame_at = Instant::now();
        } else if let Err(error) = app.draw() {
            error!(error = %error, "native renderer draw failed");
            self.exit_requested = true;
        } else {
            self.last_frame_at = Instant::now();
        }
    }

    fn handle_message(&mut self, hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = unsafe { std::mem::zeroed() };
                let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
                if RENDER_UI {
                    self.render_current_frame();
                    if self.exit_requested {
                        unsafe {
                            EndPaint(hwnd, &paint);
                        }
                        return 0;
                    }
                } else {
                    draw_shell_preview(hdc, self.host.size());
                    self.last_frame_at = Instant::now();
                }
                unsafe {
                    EndPaint(hwnd, &paint);
                }
                return 0;
            },
            WM_SIZE => {
                self.host.update_window_region();
                if let Some(runtime) = self.composition_runtime.as_mut() {
                    if let Err(error) = runtime.resize_effect_layer(self.host.size()) {
                        error!(error = %error, "composition effect layer resize failed");
                    }
                }
                if RENDER_UI {
                    if let Some(app) = self.app.as_mut() {
                        let size = self.host.size();
                        if size.width > 0 && size.height > 0 {
                            app.resize(size);
                        }
                    }
                    self.render_current_frame();
                    self.host.request_redraw();
                } else {
                    self.host.request_redraw();
                }
                return 0;
            },
            WM_NCHITTEST => {
                if let Some(hit) = self.host.hit_test_resize_border(lparam) {
                    return hit;
                }
                return HTCLIENT as LRESULT;
            },
            WM_ERASEBKGND => {
                return 1;
            },
            WM_MOUSEMOVE => {
                if RENDER_UI {
                    if let Some(app) = self.app.as_mut() {
                        let x = signed_loword(lparam) as f64;
                        let y = signed_hiword(lparam) as f64;
                        app.handle_input(InputAction::PointerMoved { x, y });
                    }
                }
                return 0;
            },
            WM_MOUSELEAVE => {
                if RENDER_UI {
                    if let Some(app) = self.app.as_mut() {
                        app.handle_input(InputAction::PointerLeft);
                    }
                }
                return 0;
            },
            WM_LBUTTONDOWN => {
                if RENDER_UI {
                    if let Some(app) = self.app.as_mut() {
                        app.handle_input(InputAction::PointerPrimaryPressed);
                        if app.close_requested() {
                            self.exit_requested = true;
                        }
                    }
                }
                return 0;
            },
            WM_KEYDOWN => {
                if RENDER_UI {
                    if let Some(app) = self.app.as_mut() {
                        if !is_repeat(lparam) {
                            if let Some(command) = map_windows_virtual_key(wparam as u32) {
                                app.handle_input(InputAction::KeyPressed(command));
                            }
                        }
                        if app.close_requested() {
                            self.exit_requested = true;
                        }
                    }
                }
                return 0;
            },
            WM_CLOSE => {
                self.exit_requested = true;
                unsafe {
                    PostQuitMessage(0);
                }
                return 0;
            },
            WM_DESTROY | WM_NCDESTROY => {
                self.exit_requested = true;
                unsafe {
                    PostQuitMessage(0);
                }
                return 0;
            },
            _ => {},
        }

        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

struct WindowsWindowHost {
    hwnd: AtomicIsize,
    hinstance: AtomicIsize,
}

impl WindowsWindowHost {
    fn new(hinstance: isize) -> Self {
        Self {
            hwnd: AtomicIsize::new(0),
            hinstance: AtomicIsize::new(hinstance),
        }
    }

    fn attach(&self, hwnd: HWND) {
        self.hwnd.store(hwnd as isize, Ordering::Release);
    }

    fn hwnd(&self) -> HWND {
        self.hwnd.load(Ordering::Acquire) as HWND
    }

    fn apply_backdrop(&self) {
        let mode = backdrop_mode();
        let applied = match mode {
            BackdropMode::LayeredAlpha => self.apply_layered_alpha(222),
            BackdropMode::SwcaAcrylic => unsafe {
                set_window_accent(self.hwnd(), ACCENT_ENABLE_ACRYLICBLURBEHIND, (18, 24, 36, 168))
            },
            BackdropMode::SwcaBlur => unsafe {
                set_window_accent(self.hwnd(), ACCENT_ENABLE_BLURBEHIND, (18, 24, 36, 168))
            },
            BackdropMode::None => false,
        };
        info!(?mode, applied, "native backdrop applied");
    }

    fn update_window_region(&self) {
        unsafe {
            let hwnd = self.hwnd();
            if hwnd.is_null() {
                return;
            }

             if !USE_NATIVE_WINDOW_REGION {
                let _ = SetWindowRgn(hwnd, null_mut(), 1);
                info!("native window region disabled; using renderer anti-aliased shell");
                return;
            }

            if IsZoomed(hwnd) != 0 {
                let _ = SetWindowRgn(hwnd, null_mut(), 1);
                return;
            }

            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return;
            }

            let width = (rect.right - rect.left).max(1);
            let height = (rect.bottom - rect.top).max(1);
            let region = CreateRoundRectRgn(
                0,
                0,
                width + 1,
                height + 1,
                WINDOW_CORNER_RADIUS * 2,
                WINDOW_CORNER_RADIUS * 2,
            );
            if !region.is_null() {
                let applied = SetWindowRgn(hwnd, region, 1) != 0;
                info!(width, height, radius = WINDOW_CORNER_RADIUS, applied, "updated native window region");
            }
        }
    }

    fn apply_layered_alpha(&self, alpha: u8) -> bool {
        unsafe {
            let ex_style = GetWindowLongPtrW(self.hwnd(), GWL_EXSTYLE);
            let _ = SetWindowLongPtrW(self.hwnd(), GWL_EXSTYLE, ex_style | WS_EX_LAYERED as isize);
            SetLayeredWindowAttributes(self.hwnd(), 0, alpha, LWA_ALPHA) != 0
        }
    }

    fn hit_test_resize_border(&self, lparam: LPARAM) -> Option<LRESULT> {
        unsafe {
            let hwnd = self.hwnd();
            if hwnd.is_null() || IsZoomed(hwnd) != 0 {
                return None;
            }

            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }

            let x = signed_loword(lparam) as i32;
            let y = signed_hiword(lparam) as i32;
            let left = x < rect.left + RESIZE_BORDER_THICKNESS;
            let right = x >= rect.right - RESIZE_BORDER_THICKNESS;
            let top = y < rect.top + RESIZE_BORDER_THICKNESS;
            let bottom = y >= rect.bottom - RESIZE_BORDER_THICKNESS;

            let hit = match (left, right, top, bottom) {
                (true, _, true, _) => HTTOPLEFT,
                (_, true, true, _) => HTTOPRIGHT,
                (true, _, _, true) => HTBOTTOMLEFT,
                (_, true, _, true) => HTBOTTOMRIGHT,
                (true, _, _, _) => HTLEFT,
                (_, true, _, _) => HTRIGHT,
                (_, _, true, _) => HTTOP,
                (_, _, _, true) => HTBOTTOM,
                _ => return None,
            };

            Some(hit as LRESULT)
        }
    }
}

impl WindowHost for WindowsWindowHost {
    fn size(&self) -> PhysicalSize<u32> {
        unsafe {
            let mut rect: RECT = std::mem::zeroed();
            if GetClientRect(self.hwnd(), &mut rect) == 0 {
                return PhysicalSize::new(1, 1);
            }
            PhysicalSize::new(
                (rect.right - rect.left).max(1) as u32,
                (rect.bottom - rect.top).max(1) as u32,
            )
        }
    }

    fn request_redraw(&self) {
        unsafe {
            InvalidateRect(self.hwnd(), null(), 0);
        }
    }

    fn start_window_drag(&self) -> Result<()> {
        unsafe {
            ReleaseCapture();
            SendMessageW(self.hwnd(), WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
        Ok(())
    }

    fn minimize(&self) {
        unsafe {
            ShowWindow(self.hwnd(), SW_MINIMIZE);
        }
    }

    fn toggle_maximize(&self) {
        unsafe {
            ShowWindow(
                self.hwnd(),
                if IsZoomed(self.hwnd()) != 0 {
                    SW_RESTORE
                } else {
                    SW_MAXIMIZE
                },
            );
        }
        self.update_window_region();
    }

    fn is_maximized(&self) -> bool {
        unsafe { IsZoomed(self.hwnd()) != 0 }
    }

    fn surface_handles(&self) -> SurfaceHandles {
        let hwnd = NonZeroIsize::new(self.hwnd.load(Ordering::Acquire)).expect("native hwnd");
        let mut window = Win32WindowHandle::new(hwnd);
        window.hinstance = NonZeroIsize::new(self.hinstance.load(Ordering::Acquire));
        SurfaceHandles {
            raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            raw_window_handle: RawWindowHandle::Win32(window),
        }
    }
}

impl HasWindowHandle for WindowsWindowHost {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.hwnd.load(Ordering::Acquire)).ok_or(HandleError::Unavailable)?;
        let mut window = Win32WindowHandle::new(hwnd);
        window.hinstance = NonZeroIsize::new(self.hinstance.load(Ordering::Acquire));
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(window)) })
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create_struct = lparam as *const CREATESTRUCTW;
        if !create_struct.is_null() {
            let shell = unsafe { (*create_struct).lpCreateParams as *mut WindowsShell };
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, shell as isize);
            }
        }
    }

    let shell_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowsShell };
    if !shell_ptr.is_null() {
        return unsafe { (*shell_ptr).handle_message(hwnd, message, wparam, lparam) };
    }

    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn signed_loword(value: LPARAM) -> i16 {
    (value & 0xffff) as u16 as i16
}

fn signed_hiword(value: LPARAM) -> i16 {
    ((value >> 16) & 0xffff) as u16 as i16
}

fn is_repeat(lparam: LPARAM) -> bool {
    (lparam & 0x4000_0000) != 0
}

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttribData {
    attrib: u32,
    pv_data: *mut c_void,
    cb_data: usize,
}

type SetWindowCompositionAttribute =
    unsafe extern "system" fn(HWND, *mut WindowCompositionAttribData) -> i32;

unsafe fn set_window_accent(hwnd: HWND, accent_state: u32, color: (u8, u8, u8, u8)) -> bool {
    let module = unsafe { LoadLibraryA(c"user32.dll".as_ptr() as _) };
    if module.is_null() {
        return false;
    }

    let proc = unsafe { GetProcAddress(module, c"SetWindowCompositionAttribute".as_ptr() as _) };
    if proc.is_none() {
        return false;
    }

    let mut rgba = color;
    if accent_state == ACCENT_ENABLE_ACRYLICBLURBEHIND && rgba.3 == 0 {
        rgba.3 = 1;
    }

    let mut policy = AccentPolicy {
        accent_state,
        accent_flags: if accent_state == ACCENT_ENABLE_ACRYLICBLURBEHIND { 0 } else { 2 },
        gradient_color: (rgba.0 as u32)
            | ((rgba.1 as u32) << 8)
            | ((rgba.2 as u32) << 16)
            | ((rgba.3 as u32) << 24),
        animation_id: 0,
    };
    let mut data = WindowCompositionAttribData {
        attrib: WCA_ACCENT_POLICY,
        pv_data: (&mut policy as *mut AccentPolicy).cast(),
        cb_data: std::mem::size_of::<AccentPolicy>(),
    };

    let set_window_composition_attribute: SetWindowCompositionAttribute =
        unsafe { std::mem::transmute(proc) };
    unsafe { set_window_composition_attribute(hwnd, &mut data as *mut _) != 0 }
}

fn backdrop_mode() -> BackdropMode {
    match std::env::var("STELLATUNE_GUI_BACKDROP")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("layered") => BackdropMode::LayeredAlpha,
        Some("acrylic") => BackdropMode::SwcaAcrylic,
        Some("blur") => BackdropMode::SwcaBlur,
        Some("none") => BackdropMode::None,
        _ => BackdropMode::None,
    }
}

fn draw_shell_preview(hdc: *mut c_void, size: PhysicalSize<u32>) {
    let width = size.width.max(1);
    let height = size.height.max(1);
    unsafe {
        let region = CreateRoundRectRgn(
            0,
            0,
            width as i32 + 1,
            height as i32 + 1,
            WINDOW_CORNER_RADIUS * 2,
            WINDOW_CORNER_RADIUS * 2,
        );
        if region.is_null() {
            return;
        }

        let fill_brush = CreateSolidBrush(rgb(10, 18, 30));
        let stroke_brush = CreateSolidBrush(rgb(210, 224, 255));
        if !fill_brush.is_null() {
            let _ = FillRgn(hdc, region, fill_brush);
        }
        if !stroke_brush.is_null() {
            let _ = FrameRgn(hdc, region, stroke_brush, 2, 2);
        }

        let _ = SetBkMode(hdc, 1);
        let _ = SetTextColor(hdc, rgb(232, 236, 245));
        let text = wide("Stellatune GUI");
        let _ = TextOutW(hdc, 20, 16, text.as_ptr(), (text.len() - 1) as i32);

        if !fill_brush.is_null() {
            let _ = DeleteObject(fill_brush as _);
        }
        if !stroke_brush.is_null() {
            let _ = DeleteObject(stroke_brush as _);
        }
        let _ = DeleteObject(region as _);
    }
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

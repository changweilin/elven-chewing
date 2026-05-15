// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{cell::RefCell, rc::Rc};

use exn::{Result, ResultExt};
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::{
        Direct2D::{
            Common::{D2D_RECT_F, D2D1_COLOR_F},
            D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
            ID2D1DeviceContext, ID2D1Factory1,
        },
        DirectComposition::IDCompositionTarget,
        DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS,
            DWriteCreateFactory, IDWriteFactory1,
        },
        Dxgi::{
            Common::DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_PRESENT, DXGI_SWAP_CHAIN_FLAG, IDXGISwapChain1,
        },
        Gdi::{BeginPaint, EndPaint, PAINTSTRUCT},
    },
    UI::WindowsAndMessaging::{
        CS_IME, GWLP_USERDATA, GetWindowLongPtrW, HWND_DESKTOP, IDC_ARROW, LoadCursorW,
        RegisterClassExW, WINDOWPOS, WM_PAINT, WM_WINDOWPOSCHANGING, WNDCLASSEXW, WS_CLIPCHILDREN,
        WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    },
};
use windows_core::{HSTRING, PCWSTR, w};

use crate::ui::{
    UiError,
    gfx::{
        clamp_point_to_monitor, create_render_target, create_swapchain, create_swapchain_bitmap,
        d3d11_device, get_dpi_for_point, get_dpi_for_window, setup_direct_composition,
    },
    message_box::draw_message_box,
    window::Window,
};

#[derive(Debug)]
pub(crate) struct DualPreview {
    model: RefCell<DualPreviewModel>,
    view: RefCell<RenderedView>,
}

#[derive(Default, Debug)]
pub(crate) struct DualPreviewModel {
    pub(crate) chinese: HSTRING,
    pub(crate) english: HSTRING,
    /// 0 = Chinese active, 1 = English active
    pub(crate) active: u8,
    pub(crate) font_family: HSTRING,
    pub(crate) font_size: f32,
    pub(crate) fg_color: D2D1_COLOR_F,
    pub(crate) bg_color: D2D1_COLOR_F,
    pub(crate) highlight_fg_color: D2D1_COLOR_F,
    pub(crate) highlight_bg_color: D2D1_COLOR_F,
    pub(crate) border_color: D2D1_COLOR_F,
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let get_this = || unsafe {
        let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const DualPreview;
        Rc::increment_strong_count(this_ptr);
        Rc::from_raw(this_ptr)
    };
    match msg {
        WM_PAINT => {
            let this = get_this();
            let view = this.view.borrow();
            let window = view.window();
            let model = this.model.borrow();
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(window.hwnd(), &mut ps) };
            let _ = view.on_paint(&model);
            let _ = unsafe { EndPaint(window.hwnd(), &ps) };
            LRESULT(0)
        }
        WM_WINDOWPOSCHANGING => {
            let pos = lparam.0 as *mut WINDOWPOS;
            if let Some(pos) = unsafe { pos.as_mut() } {
                let this = get_this();
                let view = this.view.borrow();
                let model = this.model.borrow();
                let dpi = get_dpi_for_point(POINT { x: pos.x, y: pos.y });
                if let Ok(size) = view.calculate_client_rect(&model, dpi) {
                    pos.cx = size.hw_width as i32;
                    pos.cy = size.hw_height as i32;
                    (pos.x, pos.y) = clamp_point_to_monitor(pos.x, pos.y, pos.cx, pos.cy);
                }
            }
            LRESULT(0)
        }
        _ => crate::ui::window::wnd_proc(hwnd, msg, wparam, lparam),
    }
}

#[derive(Debug)]
struct RenderedView {
    _factory: ID2D1Factory1,
    _dcomptarget: IDCompositionTarget,
    dwrite_factory: IDWriteFactory1,
    target: ID2D1DeviceContext,
    swapchain: IDXGISwapChain1,
    window: Window,
}

struct RenderedMetrics {
    width: f32,
    height: f32,
    hw_width: f32,
    hw_height: f32,
    row_height: f32,
    marker_width: f32,
    text_width: f32,
}

const ROW_SPACING: f32 = 4.0;
const MARGIN: f32 = 10.0;

impl RenderedView {
    fn new(user_data: *const DualPreview) -> Result<RenderedView, UiError> {
        let err = || UiError(format!("failed to create new RenderedView"));

        let window = Window::new();
        window.create(
            HWND_DESKTOP,
            w!("ChewingDualPreviewWindow"),
            WS_POPUP | WS_CLIPCHILDREN,
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            user_data.cast(),
        );
        unsafe {
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).or_raise(err)?;
            let dwrite_factory: IDWriteFactory1 =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).or_raise(err)?;
            let device = d3d11_device().or_raise(err)?;
            let target = create_render_target(&factory, &device).or_raise(err)?;
            let swapchain = create_swapchain(&device, 10, 10).or_raise(err)?;
            let dpi = get_dpi_for_window(window.hwnd());
            target.SetDpi(dpi, dpi);
            create_swapchain_bitmap(&swapchain, &target).or_raise(err)?;
            let dcomptarget =
                setup_direct_composition(&device, window.hwnd(), &swapchain).or_raise(err)?;
            Ok(RenderedView {
                _factory: factory,
                _dcomptarget: dcomptarget,
                dwrite_factory,
                target,
                swapchain,
                window,
            })
        }
    }
}

impl RenderedView {
    fn window(&self) -> &Window {
        &self.window
    }

    fn calculate_client_rect(
        &self,
        model: &DualPreviewModel,
        dpi: f32,
    ) -> Result<RenderedMetrics, UiError> {
        let err = || UiError(format!("failed to calculate client area"));

        let scale = dpi / 96.0;
        let text_format = unsafe {
            self.dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-TW"),
                )
                .or_raise(err)?
        };

        let marker = HSTRING::from("\u{25cf} ");
        let mut marker_metrics = DWRITE_TEXT_METRICS::default();
        let mut chinese_metrics = DWRITE_TEXT_METRICS::default();
        let mut english_metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            self.dwrite_factory
                .CreateTextLayout(&marker, &text_format, f32::MAX, f32::MAX)
                .or_raise(err)?
                .GetMetrics(&mut marker_metrics)
                .or_raise(err)?;
            self.dwrite_factory
                .CreateTextLayout(&model.chinese, &text_format, f32::MAX, f32::MAX)
                .or_raise(err)?
                .GetMetrics(&mut chinese_metrics)
                .or_raise(err)?;
            self.dwrite_factory
                .CreateTextLayout(&model.english, &text_format, f32::MAX, f32::MAX)
                .or_raise(err)?
                .GetMetrics(&mut english_metrics)
                .or_raise(err)?;
        }

        let marker_width = marker_metrics.widthIncludingTrailingWhitespace;
        let text_width = chinese_metrics
            .widthIncludingTrailingWhitespace
            .max(english_metrics.widthIncludingTrailingWhitespace);
        let row_height = chinese_metrics.height.max(english_metrics.height);
        let width = marker_width + text_width + MARGIN * 2.0;
        let height = row_height * 2.0 + ROW_SPACING + MARGIN * 2.0;

        let hw_width = (width * scale + 25.0).ceil();
        let hw_height = (height * scale + 25.0).ceil();

        Ok(RenderedMetrics {
            width,
            height,
            hw_width,
            hw_height,
            row_height,
            marker_width,
            text_width,
        })
    }

    fn on_paint(&self, model: &DualPreviewModel) -> Result<(), UiError> {
        let err = || UiError("failed to paint UI".to_string());

        if model.chinese.is_empty() && model.english.is_empty() {
            return Ok(());
        }
        let text_format = unsafe {
            self.dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-TW"),
                )
                .or_raise(err)?
        };

        let dpi = get_dpi_for_window(self.window.hwnd());
        let rm = self.calculate_client_rect(model, dpi).or_raise(err)?;
        let RenderedMetrics {
            width,
            height,
            hw_width,
            hw_height,
            row_height,
            marker_width,
            text_width,
        } = rm;
        unsafe {
            self.target.SetTarget(None);
            self.swapchain
                .ResizeBuffers(
                    0,
                    hw_width as u32,
                    hw_height as u32,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .or_raise(err)?;
            self.target.SetDpi(dpi, dpi);
        }
        create_swapchain_bitmap(&self.swapchain, &self.target).or_raise(err)?;

        let dc = &self.target;
        unsafe {
            dc.BeginDraw();

            draw_message_box(
                dc,
                0.0,
                0.0,
                width,
                height,
                model.bg_color,
                model.border_color,
            )
            .or_raise(err)?;

            let fg_brush = dc
                .CreateSolidColorBrush(&model.fg_color, None)
                .or_raise(err)?;
            let highlight_fg_brush = dc
                .CreateSolidColorBrush(&model.highlight_fg_color, None)
                .or_raise(err)?;
            let highlight_bg_brush = dc
                .CreateSolidColorBrush(&model.highlight_bg_color, None)
                .or_raise(err)?;

            let active_marker = HSTRING::from("\u{25cf} ");
            let inactive_marker = HSTRING::from("\u{25cb} ");

            // Row 0: Chinese
            // Row 1: English
            for (idx, text) in [(0u8, &model.chinese), (1u8, &model.english)] {
                let row_y = MARGIN + (idx as f32) * (row_height + ROW_SPACING);
                let is_active = model.active == idx;
                let row_rect = D2D_RECT_F {
                    left: 0.0,
                    top: row_y - ROW_SPACING / 2.0,
                    right: width,
                    bottom: row_y + row_height + ROW_SPACING / 2.0,
                };
                if is_active {
                    dc.FillRectangle(&row_rect, &highlight_bg_brush);
                }
                let marker_rect = D2D_RECT_F {
                    left: MARGIN,
                    top: row_y,
                    right: MARGIN + marker_width,
                    bottom: row_y + row_height,
                };
                let text_rect = D2D_RECT_F {
                    left: MARGIN + marker_width,
                    top: row_y,
                    right: MARGIN + marker_width + text_width,
                    bottom: row_y + row_height,
                };
                let marker_str = if is_active {
                    &active_marker
                } else {
                    &inactive_marker
                };
                let brush = if is_active {
                    &highlight_fg_brush
                } else {
                    &fg_brush
                };
                dc.DrawText(
                    marker_str,
                    &text_format,
                    &marker_rect,
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                dc.DrawText(
                    text,
                    &text_format,
                    &text_rect,
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }

            dc.EndDraw(None, None).or_raise(err)?;

            self.swapchain
                .Present(1, DXGI_PRESENT(0))
                .ok()
                .or_raise(err)?;
        }
        Ok(())
    }
}

impl DualPreview {
    pub(crate) fn window_register_class(hinst: HINSTANCE) {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_IME,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("ChewingDualPreviewWindow"),
            ..Default::default()
        };
        unsafe { RegisterClassExW(&wc) };
    }
    pub(crate) fn new() -> Result<Rc<DualPreview>, UiError> {
        let err = || UiError("failed to create dual preview window".to_string());
        let mut preview = Rc::new_uninit();
        let user_data = Rc::as_ptr(&preview);
        Rc::get_mut(&mut preview).unwrap().write(DualPreview {
            model: RefCell::new(DualPreviewModel::default()),
            view: RefCell::new(RenderedView::new(user_data.cast()).or_raise(err)?),
        });
        // SAFETY: preview is unconditionally initialized
        unsafe { Ok(preview.assume_init()) }
    }
    pub(crate) fn set_model(&self, model: DualPreviewModel) {
        *self.model.borrow_mut() = model;
    }
    pub(crate) fn set_position(&self, x: i32, y: i32) {
        let view = self.view.borrow();
        view.window().set_position(x, y);
    }
    pub(crate) fn show(&self) {
        let view = self.view.borrow();
        view.window().show();
        view.window().refresh();
    }
    pub(crate) fn hide(&self) {
        let view = self.view.borrow();
        view.window().hide();
    }
}

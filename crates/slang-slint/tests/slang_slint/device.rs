use slang_slint::ErrorKind;
use slint::GraphicsAPI;

#[test]
fn opengl_is_an_unsupported_graphics_api() {
    let get_proc_address = |_: &std::ffi::CStr| std::ptr::null();
    let api = GraphicsAPI::NativeOpenGL {
        get_proc_address: &get_proc_address,
    };
    let error = slang_slint::device(&api).expect_err("an OpenGL graphics API gave a device");
    assert_eq!(error.kind(), ErrorKind::UnsupportedGraphicsApi);
    assert_eq!(
        error.to_string(),
        "cannot create a device from the graphics API of Slint: Slint renders with \
         GraphicsAPI::NativeOpenGL and not with wgpu 30"
    );
}

#[test]
fn webgl_is_an_unsupported_graphics_api() {
    let api = GraphicsAPI::WebGL {
        canvas_element_id: "canvas",
        context_type: "webgl2",
    };
    let error = slang_slint::device(&api).expect_err("a WebGL graphics API gave a device");
    assert_eq!(error.kind(), ErrorKind::UnsupportedGraphicsApi);
}

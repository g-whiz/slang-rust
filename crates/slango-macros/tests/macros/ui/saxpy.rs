mod saxpy {
    slango_macros::shader!("../shaders/saxpy.slang");
}

fn main() {
    let load: fn(&slango::Device) -> slango::Result<saxpy::Shader> = saxpy::Shader::load;
    let _ = load;
    assert_eq!(saxpy::Saxpy::THREAD_GROUP_SIZE, [64, 1, 1]);
}

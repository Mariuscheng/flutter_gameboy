mod apu;
mod cpu;
pub mod gameboy;
mod instructions;
pub mod joypad;
mod mmu;
pub mod ppu;
mod timer;

pub mod api;
mod frb_generated;

#[cfg(target_os = "android")]
use jni::{JNIEnv, objects::JObject};

#[cfg(target_os = "android")]
use std::ffi::c_void;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_flutter_1gameboy_MainActivity_initNativeContext(
    env: JNIEnv,
    _thiz: JObject,
    context: JObject,
) {
    let vm = env
        .get_java_vm()
        .expect("failed to get JavaVM for Android context initialization");
    let context = env
        .new_global_ref(context)
        .expect("failed to create global Android context reference");

    unsafe {
        ndk_context::initialize_android_context(
            vm.get_java_vm_pointer() as *mut c_void,
            context.as_obj().as_raw() as *mut c_void,
        );
    }

    std::mem::forget(context);
}

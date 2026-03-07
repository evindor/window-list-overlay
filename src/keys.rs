use std::fs::{self, File};
use std::os::unix::io::AsRawFd;

const KEY_LEFTMETA: usize = 125;
const KEY_CNT: usize = 0x300;
const KEY_BYTES: usize = KEY_CNT.div_ceil(8);

fn ior(nr: libc::c_ulong, size: libc::c_ulong) -> libc::c_ulong {
    (2 << 30) | (size << 16) | ((b'E' as libc::c_ulong) << 8) | nr
}

fn eviocgkey() -> libc::c_ulong {
    ior(0x18, KEY_BYTES as libc::c_ulong)
}

fn eviocgbit_key() -> libc::c_ulong {
    // EVIOCGBIT(EV_KEY=1) = _IOR('E', 0x20 + 1, KEY_BYTES)
    ior(0x21, KEY_BYTES as libc::c_ulong)
}

/// Find all /dev/input/event* devices that can report KEY_LEFTMETA
pub fn find_keyboards() -> Vec<File> {
    let mut keyboards = Vec::new();
    let entries = match fs::read_dir("/dev/input/") {
        Ok(e) => e,
        Err(_) => return keyboards,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = match name.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        if !name_str.starts_with("event") {
            continue;
        }
        if let Ok(file) = File::open(entry.path()) {
            let mut caps = [0u8; KEY_BYTES];
            // SAFETY: caps is a stack-allocated buffer of KEY_BYTES; the ioctl writes
            // at most KEY_BYTES bytes into it, matching the size encoded in eviocgbit_key().
            let ret = unsafe { libc::ioctl(file.as_raw_fd(), eviocgbit_key(), caps.as_mut_ptr()) };
            if ret >= 0 && caps[KEY_LEFTMETA / 8] & (1 << (KEY_LEFTMETA % 8)) != 0 {
                keyboards.push(file);
            }
        }
    }
    keyboards
}

/// Check if left Super key is currently held down on any keyboard
pub fn is_super_pressed(keyboards: &[File]) -> bool {
    for kbd in keyboards {
        let mut keys = [0u8; KEY_BYTES];
        // SAFETY: keys is a stack-allocated buffer of KEY_BYTES; the ioctl writes
        // at most KEY_BYTES bytes into it, matching the size encoded in eviocgkey().
        let ret = unsafe { libc::ioctl(kbd.as_raw_fd(), eviocgkey(), keys.as_mut_ptr()) };
        if ret >= 0 && keys[KEY_LEFTMETA / 8] & (1 << (KEY_LEFTMETA % 8)) != 0 {
            return true;
        }
    }
    false
}

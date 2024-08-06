use std::ffi::CString;
use std::io;
use libc::{c_char, c_void, input_event, ioctl, open, read, O_RDONLY, c_int};

const TAG: &str = "[ handler.rs ]";

const EVIOCGRAB: u64 = 1074021776;

// From linux/input-event-codes.h
const EV_KEY: u16 = 0x01;
const KEY_LEFTCTRL: u16 = 29;
const KEY_CAPSLOCK: u16 = 58;
const KEY_H: u16 = 35;
const KEY_J: u16 = 36;
const KEY_K: u16 = 37;
const KEY_L: u16 = 38;
const KEY_UP: u16 = 103;
const KEY_LEFT: u16 = 105;
const KEY_RIGHT: u16 = 106;
const KEY_DOWN: u16 = 108;

pub struct KeyboardHandler {
    fd: i32,
    uinput: uinput::Device,
    is_grabbed: bool,
    debug: bool,
    device_path: String,
}

impl KeyboardHandler {
    pub fn new(device_path: &str, debug: bool) -> Result<KeyboardHandler, String> {
        let c_path = CString::new(device_path).expect("CString::new failed");
        let oflag: c_int = O_RDONLY;
        let fd: c_int;
        unsafe {
            fd = open(c_path.as_ptr(), oflag);
            println!("{} fd of open: {}", TAG, fd);
        }
        unsafe {
            // print the device path
            println!("{} device_path: {}", TAG, device_path);
            let c_path = CString::new(device_path).expect("CString::new failed");

            let fd = open(c_path.as_ptr(), O_RDONLY);
            if fd == -1 {
                panic!("{} last_os_error: {}", TAG, io::Error::last_os_error());
            }

            Ok(KeyboardHandler {
                device_path: device_path.to_string(),
                is_grabbed: false,
                uinput: uinput::open("/dev/uinput")
                    .map_err(|e| format!("Failed to create uinput device: {}", e))?
                    .name(format!("C-HJKL Output for {}", device_path))
                    .map_err(|e| format!("Failed to set uinput device name: {}", e))?
                    .event(uinput::event::Keyboard::All)
                    .map_err(|e| format!("Failed to set uinput device event: {}", e))?
                    .create()
                    .map_err(|e| format!("Failed to create uinput device: {}", e))?,
                debug,
                fd,
            })
        }
    }

    fn grab(&mut self) {
        unsafe {
            if !self.is_grabbed && ioctl(self.fd, EVIOCGRAB, 1) != -1 {
                self.is_grabbed = true;
            }
        }
    }

    #[allow(dead_code)]
    fn ungrab(&mut self) {
        unsafe {
            ioctl(self.fd, EVIOCGRAB, 0);
            self.is_grabbed = false;
        }
    }

    fn read(&self) -> input_event {
        unsafe {
            let mut ev: input_event = std::mem::zeroed();
            if read(
                self.fd,
                &mut ev as *mut _ as *mut c_void,
                std::mem::size_of::<input_event>(),
            ) != (std::mem::size_of::<input_event>() as _)
            {
                panic!("Read a partial event");
            }
            ev.clone()
        }
    }

    fn write(&mut self, ev: &input_event) {
        self.uinput
            .write(ev.type_ as _, ev.code as _, ev.value)
            .unwrap();
    }

    pub fn run_forever(&mut self) {
        let mut ctrl_pressed = false;

        std::thread::sleep(std::time::Duration::from_secs(1));

        self.grab();
        loop {
            let mut input = self.read();

            if self.debug {
                println!(
                    "[{}] ctrl:{}, ev: {} {} {}",
                    self.device_path, ctrl_pressed, input.type_, input.code, input.value
                );
            }

            // Handle Capslock / Ctrl
            if input.type_ == EV_KEY && input.code == KEY_CAPSLOCK {
                input.code = KEY_LEFTCTRL;
            }

            // Maintain Ctrl flag
            if input.type_ == EV_KEY && input.code == KEY_LEFTCTRL {
                ctrl_pressed = input.value != 0;
            }

            // Handle C-hjkl
            if input.type_ == EV_KEY && input.value >= 1 && ctrl_pressed {
                let key_to_press = if input.code == KEY_H {
                    KEY_LEFT
                } else if input.code == KEY_J {
                    KEY_DOWN
                } else if input.code == KEY_K {
                    KEY_UP
                } else if input.code == KEY_L {
                    KEY_RIGHT
                } else {
                    0
                };

                if key_to_press > 0 {
                    input.value = 0;
                    input.code = KEY_LEFTCTRL;
                    self.write(&input);

                    input.code = key_to_press;
                    input.value = 1;
                    self.write(&input);

                    input.value = 0;
                    self.write(&input);

                    input.value = 1;
                    input.code = KEY_LEFTCTRL;
                    self.write(&input);

                    continue;
                }
            }

            // Pass-through
            self.write(&input);
        }
    }
}

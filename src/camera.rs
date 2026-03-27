// camera.rs — Camera capture and image handling module

use std::path::PathBuf;
use std::process::Command;
use std::error::Error;

/// Get list of available camera devices by testing them with FFmpeg
pub fn get_camera_devices() -> Vec<(usize, String)> {
    let mut cameras = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // On macOS, enumerate AVFoundation devices by testing them
        for i in 0..5 {
            let device_id = format!("{}:0", i);
            let output = Command::new("ffmpeg")
                .arg("-f").arg("avfoundation")
                .arg("-framerate").arg("30")
                .arg("-i").arg(&device_id)
                .arg("-frames:v").arg("1")
                .arg("-t").arg("0.1")
                .arg("-y")
                .arg("/dev/null")
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .output();

            if let Ok(status) = output {
                if status.status.success() {
                    cameras.push((i, format!("Camera {}", i)));
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, check /dev/video devices directly
        for i in 0..10 {
            let device = format!("/dev/video{}", i);
            if std::path::Path::new(&device).exists() {
                // Verify it's actually a working camera by trying to query it
                let test = Command::new("v4l2-ctl")
                    .arg("-d").arg(&device)
                    .arg("--info")
                    .stderr(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .output();

                if test.is_ok() {
                    cameras.push((i, format!("Camera {}", i)));
                } else {
                    // Fallback: if v4l2-ctl is not available, still add the device
                    cameras.push((i, format!("Camera {}", i)));
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, try to enumerate cameras through dshow
        for i in 0..5 {
            let device = format!("video=\"Camera {}\"", i);
            let output = Command::new("ffmpeg")
                .arg("-f").arg("dshow")
                .arg("-i").arg(&device)
                .arg("-frames:v").arg("1")
                .arg("-t").arg("0.1")
                .arg("-y")
                .arg("nul")
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .output();

            if let Ok(status) = output {
                if status.status.success() {
                    cameras.push((i, format!("Camera {}", i)));
                }
            }
        }
    }

    // Always provide at least Camera 0 as fallback
    if cameras.is_empty() {
        cameras.push((0, "Camera 0".to_string()));
    }

    cameras
}

/// Capture a frame from the specified camera using FFmpeg
pub fn capture_frame(camera_index: usize) -> Result<PathBuf, Box<dyn Error>> {
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let output_path = temp_dir.join(format!("camera_capture_{}.jpg", timestamp));

    #[cfg(target_os = "macos")]
    {
        // Use FFmpeg to capture from AVFoundation device on macOS
        let device_id = format!("{}:0", camera_index);
        let output = Command::new("ffmpeg")
            .arg("-f")
            .arg("avfoundation")
            .arg("-framerate")
            .arg("30")
            .arg("-i")
            .arg(&device_id)
            .arg("-vframes")
            .arg("1")
            .arg("-y")
            .arg(output_path.to_str().unwrap())
            .output()?;

        if !output.status.success() {
            return Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use FFmpeg to capture from /dev/video device on Linux
        let device = format!("/dev/video{}", camera_index);
        let output = Command::new("ffmpeg")
            .arg("-f")
            .arg("v4l2")
            .arg("-i")
            .arg(&device)
            .arg("-vframes")
            .arg("1")
            .arg("-y")
            .arg(output_path.to_str().unwrap())
            .output()?;

        if !output.status.success() {
            return Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Use FFmpeg to capture from dshow device on Windows
        let device = format!("video=\"Camera {}\"", camera_index);
        let output = Command::new("ffmpeg")
            .arg("-f")
            .arg("dshow")
            .arg("-i")
            .arg(&device)
            .arg("-vframes")
            .arg("1")
            .arg("-y")
            .arg(output_path.to_str().unwrap())
            .output()?;

        if !output.status.success() {
            return Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
    }

    Ok(output_path)
}

/// Load image from file path and return as bytes for display
pub fn load_image_for_display(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let img = image::open(path)?;
    let rgb_img = img.to_rgb8();

    Ok(rgb_img.to_vec())
}

/// Get image dimensions
pub fn get_image_dimensions(path: &str) -> Result<(u32, u32), Box<dyn Error>> {
    let img = image::open(path)?;
    Ok((img.width(), img.height()))
}

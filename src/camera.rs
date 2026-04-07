// camera.rs — Camera capture and image handling module

use std::path::PathBuf;
use std::process::Command;
use std::error::Error;

/// Get list of available camera devices by querying AVFoundation
pub fn get_camera_devices() -> Vec<(usize, String)> {
    let mut cameras = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // On macOS, use ffmpeg -list_devices to get actual device names
        let output = Command::new("ffmpeg")
            .arg("-f").arg("avfoundation")
            .arg("-list_devices").arg("true")
            .arg("-i").arg("")
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .output();

        if let Ok(output) = output {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = stderr.to_string();

            eprintln!("[DEBUG] FFmpeg device list output:\n{}", combined);

            // Parse AVFoundation video device list
            // Format: [AVFoundation indev @ ...] [0] FaceTime HD Camera
            // Note: We need to be in video section before audio section
            let mut in_video_section = false;

            for line in combined.lines() {
                // Look for the start of the video section
                if line.contains("AVFoundation video devices") {
                    in_video_section = true;
                    continue;
                }

                // Stop when we hit audio devices
                if line.contains("AVFoundation audio devices") {
                    in_video_section = false;
                    break;
                }

                // In video section, look for device lines with pattern: [...] [index] device_name
                if in_video_section && line.contains("[AVFoundation indev") {
                    // Extract the pattern [n] device_name after the address
                    // The line looks like: [AVFoundation indev @ 0x...] [0] FaceTime HD Camera
                    if let Some(last_bracket_start) = line.rfind('[') {
                        if let Some(last_bracket_end) = line[last_bracket_start..].find(']') {
                            let idx_part = &line[last_bracket_start + 1..last_bracket_start + last_bracket_end];

                            // Try to parse the index
                            if let Ok(device_index) = idx_part.trim().parse::<usize>() {
                                // Get device name after the closing bracket
                                let device_name_start = last_bracket_start + last_bracket_end + 1;
                                if device_name_start < line.len() {
                                    let device_name = line[device_name_start..].trim().to_string();

                                    if !device_name.is_empty() && !device_name.contains("audio") {
                                        eprintln!("[DEBUG] Found device: index={}, name={}", device_index, device_name);
                                        cameras.push((device_index, device_name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: test devices if list_devices didn't work
        if cameras.is_empty() {
            eprintln!("[DEBUG] Device list parsing failed, trying fallback device detection");
            for i in 0..10 {
                let device_id = format!("{}:0", i);
                eprintln!("[DEBUG] Testing device: {}", device_id);

                // Use a simpler test - just try to open without capturing
                let test = Command::new("ffmpeg")
                    .arg("-f").arg("avfoundation")
                    .arg("-framerate").arg("30")
                    .arg("-i").arg(&device_id)
                    .arg("-frames:v").arg("1")
                    .arg("-update").arg("1")
                    .arg("-t").arg("0.5")
                    .arg("-y")
                    .arg("/tmp/camera_test_null.jpg")
                    .stderr(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .output();

                if let Ok(output) = test {
                    if output.status.success() || !output.status.success() {
                        // Device exists if it doesn't error out immediately
                        // (AVFoundation might take time to initialize)
                        cameras.push((i, format!("Camera {}", i)));
                        eprintln!("[DEBUG] Device {} available", i);
                    }
                }
            }

            // Clean up test file
            let _ = std::fs::remove_file("/tmp/camera_test_null.jpg");
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

    eprintln!("[DEBUG] Capturing frame from camera {} to {:?}", camera_index, output_path);

    #[cfg(target_os = "macos")]
    {
        // Use FFmpeg to capture from AVFoundation device on macOS
        let device_id = format!("{}:0", camera_index);

        // Try different framerates since some devices (like OBS virtual camera)
        // only support specific framerates (e.g., 60fps)
        let framerates = vec!["60", "30", "24", "15"];
        let mut capture_succeeded = false;
        let mut last_error = String::new();

        for &framerate in &framerates {
            eprintln!("[DEBUG] Attempting capture with framerate: {}fps", framerate);

            let mut cmd = Command::new("ffmpeg");
            cmd.arg("-f").arg("avfoundation")
                .arg("-framerate").arg(framerate)
                .arg("-i").arg(&device_id)
                .arg("-vframes").arg("1")
                .arg("-vf").arg("scale=1280:720,pad=1280:720:(ow-iw)/2:(oh-ih)/2")  // 1280x720 resolution
                .arg("-pix_fmt").arg("yuv420p")  // Use standard JPG-compatible pixel format
                .arg("-update").arg("1")
                .arg("-y")
                .arg(output_path.to_str().unwrap());

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        eprintln!("[DEBUG] Capture succeeded with {}fps framerate", framerate);
                        capture_succeeded = true;
                        break;
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        last_error = stderr.to_string();

                        // Check if it's a framerate-related error
                        if stderr.contains("not supported by the device") ||
                           stderr.contains("Supported modes") ||
                           stderr.contains("Error opening input") {
                            eprintln!("[DEBUG] Framerate {}fps not supported, trying next", framerate);
                            continue;
                        } else {
                            // Different kind of error, might not be framerate-related
                            eprintln!("[DEBUG] Capture failed with framerate {}fps: {}", framerate, stderr);
                            last_error = stderr.to_string();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[DEBUG] Command execution error with framerate {}fps: {}", framerate, e);
                    last_error = e.to_string();
                }
            }
        }

        // If all framerates failed, try without specifying framerate (let FFmpeg negotiate)
        if !capture_succeeded {
            eprintln!("[DEBUG] All explicit framerates failed, trying without framerate specification");

            let mut cmd = Command::new("ffmpeg");
            cmd.arg("-f").arg("avfoundation")
                .arg("-i").arg(&device_id)
                .arg("-vframes").arg("1")
                .arg("-vf").arg("scale=1280:720,pad=1280:720:(ow-iw)/2:(oh-ih)/2")  // 1280x720 resolution
                .arg("-pix_fmt").arg("yuv420p")  // Use standard JPG-compatible pixel format
                .arg("-update").arg("1")
                .arg("-y")
                .arg(output_path.to_str().unwrap());

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        eprintln!("[DEBUG] Capture succeeded without explicit framerate");
                        capture_succeeded = true;
                    } else {
                        last_error = String::from_utf8_lossy(&output.stderr).to_string();
                        eprintln!("[DEBUG] Capture failed without explicit framerate: {}", last_error);
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    eprintln!("[DEBUG] Command execution error: {}", e);
                }
            }
        }

        if !capture_succeeded {
            eprintln!("[DEBUG] All capture attempts failed");
            eprintln!("[DEBUG] Last error: {}", last_error);
            return Err(format!("FFmpeg failed after trying multiple framerates: {}", last_error).into());
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use FFmpeg to capture from /dev/video device on Linux
        let device = format!("/dev/video{}", camera_index);
        let output = Command::new("ffmpeg")
            .arg("-f")
            .arg("v4l2")
            .arg("-framerate").arg("30")
            .arg("-i")
            .arg(&device)
            .arg("-vframes")
            .arg("1")
            .arg("-pix_fmt").arg("rgb24")
            .arg("-y")
            .arg(output_path.to_str().unwrap())
            .output()?;

        if !output.status.success() {
            // Try without framerate specification
            let output2 = Command::new("ffmpeg")
                .arg("-f")
                .arg("v4l2")
                .arg("-i")
                .arg(&device)
                .arg("-vframes")
                .arg("1")
                .arg("-pix_fmt").arg("rgb24")
                .arg("-y")
                .arg(output_path.to_str().unwrap())
                .output()?;

            if !output2.status.success() {
                return Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output2.stderr)).into());
            }
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
            .arg("-update").arg("1")
            .arg("-pix_fmt").arg("rgb24")
            .arg("-y")
            .arg(output_path.to_str().unwrap())
            .output()?;

        if !output.status.success() {
            return Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
    }

    // Verify file was created
    if !output_path.exists() {
        return Err(format!("Capture file was not created: {:?}", output_path).into());
    }

    // Check file size
    let file_size = std::fs::metadata(&output_path)?.len();
    eprintln!("[DEBUG] Captured file size: {} bytes", file_size);

    if file_size == 0 {
        return Err("Captured file is empty".into());
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

// Global configuration constants for fuzzy inference system

// Defuzzification: time range and integration step
pub const T_MIN: f64 = 0.0;   // minimum extension time (seconds)
pub const T_MAX: f64 = 60.0;  // maximum extension time (seconds)
pub const STEP:  f64 = 0.1;   // numerical integration step size (seconds)


// Traffic light basic configuration
pub const PHASE_BASIC_PED_GREEN: f64 = 30.0; // all values in seconds
pub const PHASE_BASIC_VEH_GREEN: f64 = 15.0;
pub const PED_GREEN_BLINKING: f64 = 7.0;
pub const VEH_GREEN_BLINKING: f64 = 10.0;
pub const VEH_YELLOW: f64 = 3.0;
pub const ALL_RED: f64 = 3.0;

// Weight of Extend Time and Historical avg Extend Time from DB
pub const CURRENT_EXTEND_TIME_WEIGHT: f64 = 0.6;    // 0.6 current 0.4 historical

// Database configuration
pub const SQL_FILE_LOCATION: &str = "./sqheavy/db.sqlite";

// Python and ML model configuration
pub const PYTHON_VENV_SITE_PACKAGES: &str = "python/.venv/lib/python3.9/site-packages";
pub const PYTHON_DIR: &str = "python";
pub const YOLO_MODEL_PATH: &str = "models/yolov8n.pt";

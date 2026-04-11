use pyo3::prelude::*;
use std::path::PathBuf;
use crate::config::{PYTHON_VENV_SITE_PACKAGES, PYTHON_DIR, YOLO_MODEL_PATH};

fn setup_python_env(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let sys = py.import_bound("sys")?;
    let path = sys.getattr("path")?;

    let venv_site = manifest_dir.join(PYTHON_VENV_SITE_PACKAGES);
    path.call_method1("insert", (0, venv_site.to_str().unwrap()))?;

    let python_dir = manifest_dir.join(PYTHON_DIR);
    path.call_method1("append", (python_dir.to_str().unwrap(),))?;

    // Set the model path as a module-level variable in Python
    let model_path = manifest_dir.join(YOLO_MODEL_PATH);
    let count_box = py.import_bound("count_box")?;
    count_box.setattr("model_path", model_path.to_str().unwrap())?;

    Ok(count_box)
}

pub fn count_people(image_path: &str) -> PyResult<i32> {
    Python::with_gil(|py| {
        let count_box = setup_python_env(py)?;
        count_box
            .getattr("count_boxes_people")?
            .call1((image_path,))?
            .extract()
    })
}

pub fn count_cars(image_path: &str) -> PyResult<i32> {
    Python::with_gil(|py| {
        let count_box = setup_python_env(py)?;
        count_box
            .getattr("count_boxes_cars")?
            .call1((image_path,))?
            .extract()
    })
}

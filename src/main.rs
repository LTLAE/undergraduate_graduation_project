use pyo3::prelude::*;
use std::path::PathBuf;

fn main() -> PyResult<()> {
    Python::with_gil(|py| {
        // Get root dir
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // add venv/site-packages >> sys.path, aka import dependencies
        let sys = py.import_bound("sys")?;
        let path = sys.getattr("path")?;

        let venv_site = manifest_dir.join("python/.venv/lib/python3.9/site-packages");
        path.call_method1("insert", (0, venv_site.to_str().unwrap()))?;

        // add python(dir) >> sys.path, aka import python scripts
        let python_dir = manifest_dir.join("python");
        path.call_method1("append", (python_dir.to_str().unwrap(),))?;

        // add count_box.py
        let count_box = py.import_bound("count_box")?;

        let image_path = manifest_dir.join("python/test_images/153401798.jpg");
        let image_path_str = image_path.to_str().unwrap();

        println!("Recognizing image: {}", image_path_str);
        println!("----------------------------------------");

        // run count_boxes_people()
        let count_people: i32 = count_box
            .getattr("count_boxes_people")?
            .call1((image_path_str,))?
            .extract()?;

        println!("People count: {}", count_people);
        Ok(())
    })
}

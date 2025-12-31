from ultralytics import YOLO

model = YOLO('models/yolov8n.pt')

CONFIDENCE: float = 0.25
IOU: float = 0.45

CLASS_PERSON: int = 0
CLASS_CAR: int = 2

def count_boxes_people(image_path: str) -> int:
    results = model.predict(source=image_path, conf=CONFIDENCE, iou=IOU)
    boxes = results[0].boxes
    person_boxes = [b for b in boxes if int(b.cls) == CLASS_PERSON and b.conf >= CONFIDENCE]
    return len(person_boxes)

def count_boxes_cars(image_path: str) -> int:
    results = model.predict(source=image_path, conf=CONFIDENCE, iou=IOU)
    boxes = results[0].boxes
    car_boxes = [b for b in boxes if int(b.cls) == CLASS_CAR and b.conf >= CONFIDENCE]
    return len(car_boxes)

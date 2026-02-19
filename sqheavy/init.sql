-- force foreign keys on
PRAGMA foreign_keys = ON;

-- tables
CREATE TABLE IF NOT EXISTS vehicle_fuzzy_results (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp    INTEGER NOT NULL,
  fuzzy_result TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS pedestrian_fuzzy_results (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp    INTEGER NOT NULL,
  fuzzy_result TEXT    NOT NULL
);

-- index by timestamp... as usual
CREATE INDEX IF NOT EXISTS idx_vehicle_fuzzy_results_timestamp
  ON vehicle_fuzzy_results(timestamp);

CREATE INDEX IF NOT EXISTS idx_pedestrian_fuzzy_results_timestamp
  ON pedestrian_fuzzy_results(timestamp);


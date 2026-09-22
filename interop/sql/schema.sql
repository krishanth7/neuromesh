-- Store observed conformance outcomes; uniqueness prevents double-counting a case.
CREATE TABLE conformance (
    language TEXT NOT NULL,
    case_id INTEGER NOT NULL CHECK (case_id >= 0),
    expected_valid INTEGER NOT NULL CHECK (expected_valid IN (0, 1)),
    passed INTEGER NOT NULL CHECK (passed IN (0, 1)),
    PRIMARY KEY (language, case_id)
);

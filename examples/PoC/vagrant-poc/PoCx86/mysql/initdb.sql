CREATE USER 'dbint'@'%' IDENTIFIED BY 'raspberry';
    GRANT ALL PRIVILEGES ON * . * TO 'dbint'@'%';
    CREATE DATABASE IF NOT EXISTS PoC_Registration;

    CREATE TABLE patient (
        id INTEGER NOT NULL AUTO_INCREMENT,
        firstname VARCHAR(80) NOT NULL,
        lastname VARCHAR(80) NOT NULL,
        gender ENUM('male','female','unspecified') NOT NULL,
        ethnicity ENUM('white','hispanic_latino','black_african_american','native_american_american_indian','asian_pacific_islander','other') NOT NULL,
        birthdate DATE NOT NULL,
        PRIMARY KEY (id)
    );

    COMMIT;

    CREATE TABLE bloodpressure (
        id INTEGER NOT NULL AUTO_INCREMENT,
        systolic INTEGER NOT NULL,
        diastolic INTEGER NOT NULL,
        meanap INTEGER NOT NULL,
        pulserate INTEGER NOT NULL,
        date DATETIME NOT NULL,
        patient_id INTEGER NOT NULL,
        PRIMARY KEY (id),
        FOREIGN KEY(patient_id) REFERENCES patient (id)
    );

    COMMIT;

    CREATE TABLE weight (
        id INTEGER NOT NULL AUTO_INCREMENT,
        weight INTEGER NOT NULL,
        date DATETIME NOT NULL,
        patient_id INTEGER NOT NULL,
        PRIMARY KEY (id),
        FOREIGN KEY(patient_id) REFERENCES patient (id)
    );

    COMMIT;

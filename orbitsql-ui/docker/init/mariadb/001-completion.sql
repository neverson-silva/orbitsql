CREATE TABLE completion_users (
  id integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  email varchar(255) NOT NULL UNIQUE,
  status boolean NOT NULL DEFAULT true,
  status_enum enum('active', 'inactive') NOT NULL DEFAULT 'active',
  status_label varchar(16) GENERATED ALWAYS AS (IF(status, 'active', 'inactive')) STORED
);

CREATE TABLE completion_orders (
  id integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id integer NOT NULL,
  total decimal(12, 2) NOT NULL,
  CONSTRAINT completion_orders_user_fk FOREIGN KEY (user_id) REFERENCES completion_users(id),
  CONSTRAINT completion_orders_total_check CHECK (total >= 0)
);

CREATE VIEW completion_users_view AS SELECT id, email, status FROM completion_users;

CREATE SEQUENCE completion_sequence;

DELIMITER $$
CREATE FUNCTION completion_identity(value_text varchar(255)) RETURNS varchar(255) DETERMINISTIC RETURN value_text$$
CREATE PROCEDURE completion_noop(IN input_value varchar(255), OUT output_value varchar(255), INOUT inout_value varchar(255)) SELECT input_value, inout_value INTO output_value, inout_value$$
DELIMITER ;

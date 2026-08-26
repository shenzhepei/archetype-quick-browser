CREATE TABLE IF NOT EXISTS orders (
  id CHAR(36) PRIMARY KEY,
  status ENUM('available', 'claimed') NOT NULL,
  claimed_by VARCHAR(190),
  claimed_at TIMESTAMP(3) NULL,
  created_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3)
);

CREATE TABLE IF NOT EXISTS inventory_reservations (
  order_id CHAR(36) PRIMARY KEY,
  user_id VARCHAR(190) NOT NULL,
  created_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  CONSTRAINT inventory_order_fk FOREIGN KEY (order_id) REFERENCES orders(id)
);

INSERT IGNORE INTO orders (id, status) VALUES ('11111111-1111-4111-8111-111111111111', 'available');

-- 0003_add_updated_at_triggers.sql
-- Keep updated_at current on airports, airlines, and flights, matching the
-- behaviour users already has. The set_updated_at() function comes from
-- 0001_create_users.sql.

DROP TRIGGER IF EXISTS airports_set_updated_at ON airports;
CREATE TRIGGER airports_set_updated_at
BEFORE UPDATE ON airports
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS airlines_set_updated_at ON airlines;
CREATE TRIGGER airlines_set_updated_at
BEFORE UPDATE ON airlines
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS flights_set_updated_at ON flights;
CREATE TRIGGER flights_set_updated_at
BEFORE UPDATE ON flights
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

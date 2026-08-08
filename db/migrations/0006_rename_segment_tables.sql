-- 0006_rename_segment_tables.sql
-- Rename the segment tables onto a `journey_` prefix.
--
--   journey_segments -> journey_legs
--   segment_flights  -> journey_flights
--   segment_drives   -> journey_drives
--
-- A rename in Postgres carries the table's data, indexes, constraints, foreign
-- keys, and triggers with it, but leaves all of them under their old *names*.
-- So each is renamed too — otherwise `journey_legs` would keep answering to
-- `journey_segments_pkey`, and the next person to read an error message would
-- be hunting a table that no longer exists.
--
-- `journey_totals` needs no attention: a view stores resolved references, not
-- text, so it follows the rename on its own.
--
-- Transactional, like 0004 and 0005: whole, or not at all.

BEGIN;

-- ---------------------------------------------------------------------------
-- The tables
-- ---------------------------------------------------------------------------

ALTER TABLE journey_segments RENAME TO journey_legs;
ALTER TABLE segment_flights  RENAME TO journey_flights;
ALTER TABLE segment_drives   RENAME TO journey_drives;

-- The child tables point at a leg now, not a segment.
ALTER TABLE journey_flights RENAME COLUMN segment_id TO leg_id;
ALTER TABLE journey_drives  RENAME COLUMN segment_id TO leg_id;

-- ---------------------------------------------------------------------------
-- journey_legs — constraints, indexes, trigger
-- ---------------------------------------------------------------------------

ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_pkey
    TO journey_legs_pkey;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_position_key
    TO journey_legs_position_key;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_mode_key
    TO journey_legs_mode_key;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_mode_check
    TO journey_legs_mode_check;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_position_check
    TO journey_legs_position_check;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_window_check
    TO journey_legs_window_check;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_duration_check
    TO journey_legs_duration_check;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_distance_check
    TO journey_legs_distance_check;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_endpoints_check
    TO journey_legs_endpoints_check;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_journey_id_fkey
    TO journey_legs_journey_id_fkey;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_origin_place_id_fkey
    TO journey_legs_origin_place_id_fkey;
ALTER TABLE journey_legs RENAME CONSTRAINT journey_segments_destination_place_id_fkey
    TO journey_legs_destination_place_id_fkey;

ALTER INDEX idx_journey_segments_journey_position
    RENAME TO idx_journey_legs_journey_position;
ALTER INDEX idx_journey_segments_mode        RENAME TO idx_journey_legs_mode;
ALTER INDEX idx_journey_segments_started     RENAME TO idx_journey_legs_started;
ALTER INDEX idx_journey_segments_origin      RENAME TO idx_journey_legs_origin;
ALTER INDEX idx_journey_segments_destination RENAME TO idx_journey_legs_destination;

ALTER TRIGGER journey_segments_set_updated_at ON journey_legs
    RENAME TO journey_legs_set_updated_at;

-- ---------------------------------------------------------------------------
-- journey_flights — constraints, index, trigger
-- ---------------------------------------------------------------------------

ALTER TABLE journey_flights RENAME CONSTRAINT segment_flights_pkey
    TO journey_flights_pkey;
ALTER TABLE journey_flights RENAME CONSTRAINT segment_flights_segment_fkey
    TO journey_flights_leg_fkey;
ALTER TABLE journey_flights RENAME CONSTRAINT segment_flights_flight_id_fkey
    TO journey_flights_flight_id_fkey;
ALTER TABLE journey_flights RENAME CONSTRAINT segment_flights_mode_check
    TO journey_flights_mode_check;
ALTER TABLE journey_flights RENAME CONSTRAINT segment_flights_cabin_check
    TO journey_flights_cabin_check;

ALTER INDEX idx_segment_flights_flight RENAME TO idx_journey_flights_flight;

ALTER TRIGGER segment_flights_set_updated_at ON journey_flights
    RENAME TO journey_flights_set_updated_at;

-- ---------------------------------------------------------------------------
-- journey_drives — constraints, index, trigger
-- ---------------------------------------------------------------------------

ALTER TABLE journey_drives RENAME CONSTRAINT segment_drives_pkey
    TO journey_drives_pkey;
ALTER TABLE journey_drives RENAME CONSTRAINT segment_drives_segment_fkey
    TO journey_drives_leg_fkey;
ALTER TABLE journey_drives RENAME CONSTRAINT segment_drives_vehicle_id_fkey
    TO journey_drives_vehicle_id_fkey;
ALTER TABLE journey_drives RENAME CONSTRAINT segment_drives_mode_check
    TO journey_drives_mode_check;
ALTER TABLE journey_drives RENAME CONSTRAINT segment_drives_role_check
    TO journey_drives_role_check;

ALTER INDEX idx_segment_drives_vehicle RENAME TO idx_journey_drives_vehicle;

ALTER TRIGGER segment_drives_set_updated_at ON journey_drives
    RENAME TO journey_drives_set_updated_at;

COMMIT;

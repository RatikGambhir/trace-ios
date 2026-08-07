-- 0004_create_journeys_schema.sql
-- Journeys: a user's trip, made of ordered segments, each travelled by some
-- mode of transport.
--
-- The shape is a shared parent (`journey_segments`) plus one small table per
-- mode that needs extra columns. Everything every mode has — when, where, how
-- long, how far — lives on the parent, so a journey total is one SUM across
-- every mode. Each subtype table foreign-keys `(id, mode)` back to the parent,
-- which makes it structurally impossible to hang a drive off a flight segment.
--
-- Adding a mode later is one new table plus one value in the `mode` CHECK. No
-- existing row changes, and no nullable column is added to anything.
--
-- Wrapped in a transaction because Postgres DDL is transactional: applied by a
-- bare `psql -f`, which is otherwise autocommit, a failure half-way would leave
-- a partial schema. This way it either lands whole or not at all, and a failed
-- run can simply be repeated.

BEGIN;

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------------------
-- places — any endpoint of a segment
-- ---------------------------------------------------------------------------
-- `airports` only covers one kind of endpoint; a drive goes from a house to a
-- trailhead. Every marker the globe view draws comes from here, whatever the
-- mode of the leg that touched it.

CREATE TABLE places (
    id              BIGSERIAL PRIMARY KEY,

    name            VARCHAR(200) NOT NULL,
    kind            VARCHAR(20) NOT NULL DEFAULT 'other'
        CHECK (kind IN ('airport', 'station', 'port', 'address', 'city', 'landmark', 'other')),

    -- Set only for airport places, and then it is the canonical airport row.
    -- Kept in step by `sync_airport_place()` below.
    airport_id      BIGINT UNIQUE REFERENCES airports(id) ON DELETE CASCADE,

    address         TEXT,
    city            VARCHAR(100),
    country_code    CHAR(2),
    latitude        DECIMAL(9, 6),
    longitude       DECIMAL(9, 6),
    timezone        VARCHAR(50),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- `kind = 'airport'` and `airport_id IS NOT NULL` mean the same thing, so
    -- neither can be true without the other.
    CONSTRAINT places_airport_kind_check
        CHECK ((kind = 'airport') = (airport_id IS NOT NULL)),

    -- A lone latitude is useless: no distance, no marker.
    CONSTRAINT places_coordinates_paired_check
        CHECK ((latitude IS NULL) = (longitude IS NULL)),
    CONSTRAINT places_latitude_range_check
        CHECK (latitude IS NULL OR latitude BETWEEN -90 AND 90),
    CONSTRAINT places_longitude_range_check
        CHECK (longitude IS NULL OR longitude BETWEEN -180 AND 180)
);

CREATE INDEX idx_places_kind ON places (kind);
CREATE INDEX idx_places_city ON places (city);

-- Mirror every airport into `places`, now and from here on.
--
-- Doing it as a trigger rather than in application code keeps the invariant
-- true no matter which path inserts the airport — including the get-or-create
-- inside POST /api/v1/flights, where it runs in that request's transaction and
-- rolls back with it.
CREATE OR REPLACE FUNCTION sync_airport_place()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO places (
        name, kind, airport_id, city, country_code, latitude, longitude, timezone
    )
    VALUES (
        NEW.name, 'airport', NEW.id, NEW.city, NEW.country_code,
        NEW.latitude, NEW.longitude, NEW.timezone
    )
    ON CONFLICT (airport_id) DO UPDATE SET
        name         = EXCLUDED.name,
        city         = EXCLUDED.city,
        country_code = EXCLUDED.country_code,
        latitude     = EXCLUDED.latitude,
        longitude    = EXCLUDED.longitude,
        timezone     = EXCLUDED.timezone;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS airports_sync_place ON airports;
CREATE TRIGGER airports_sync_place
AFTER INSERT OR UPDATE ON airports
FOR EACH ROW
EXECUTE FUNCTION sync_airport_place();

-- Backfill the airports that existed before the trigger did.
INSERT INTO places (name, kind, airport_id, city, country_code, latitude, longitude, timezone)
SELECT name, 'airport', id, city, country_code, latitude, longitude, timezone
FROM airports
ON CONFLICT (airport_id) DO NOTHING;

-- ---------------------------------------------------------------------------
-- vehicles — "the car used", stored once and reused
-- ---------------------------------------------------------------------------

CREATE TABLE vehicles (
    id              BIGSERIAL PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    nickname        VARCHAR(100),
    make            VARCHAR(50),
    model           VARCHAR(50),
    year            SMALLINT,
    license_plate   VARCHAR(20),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- A vehicle with nothing to call it is not worth a row.
    CONSTRAINT vehicles_identifiable_check
        CHECK (nickname IS NOT NULL OR make IS NOT NULL OR model IS NOT NULL),

    CONSTRAINT vehicles_year_check
        CHECK (year IS NULL OR year BETWEEN 1885 AND 2100)
);

CREATE INDEX idx_vehicles_user ON vehicles (user_id);

-- ---------------------------------------------------------------------------
-- journeys — one trip belonging to one user
-- ---------------------------------------------------------------------------

CREATE TABLE journeys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    title           VARCHAR(200) NOT NULL,
    description     TEXT,

    -- The window the user declares. Nullable so a trip can be sketched before
    -- it has any segments; `journey_totals` reports the actual span.
    started_at      TIMESTAMPTZ,
    ended_at        TIMESTAMPTZ,

    status          VARCHAR(20) NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'active', 'completed', 'cancelled')),
    visibility      VARCHAR(20) NOT NULL DEFAULT 'private'
        CHECK (visibility IN ('private', 'friends', 'public')),

    -- For genuinely open-ended extras. Anything worth filtering or sorting on
    -- belongs in a column instead.
    metadata        JSONB NOT NULL DEFAULT '{}',

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT journeys_window_check
        CHECK (started_at IS NULL OR ended_at IS NULL OR ended_at >= started_at)
);

CREATE INDEX idx_journeys_user_started ON journeys (user_id, started_at DESC);
CREATE INDEX idx_journeys_status ON journeys (status);
CREATE INDEX idx_journeys_visibility ON journeys (visibility);

-- ---------------------------------------------------------------------------
-- journey_segments — one leg, whatever carried it
-- ---------------------------------------------------------------------------

CREATE TABLE journey_segments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    journey_id      UUID NOT NULL REFERENCES journeys(id) ON DELETE CASCADE,

    -- Order within the journey. Gaps are fine; only the ordering matters.
    position        INTEGER NOT NULL,

    mode            VARCHAR(20) NOT NULL
        CHECK (
            mode IN (
                'flight',
                'drive',
                'train',
                'bus',
                'ferry',
                'walk',
                'bike',
                'other'
            )
        ),

    origin_place_id         BIGINT REFERENCES places(id),
    destination_place_id    BIGINT REFERENCES places(id),

    started_at              TIMESTAMPTZ,
    ended_at                TIMESTAMPTZ,

    -- Held separately from the timestamps: a drive can have a known duration
    -- and vague clock times, or exact times and stops that make the two
    -- disagree on purpose.
    duration_minutes        INTEGER,
    distance_miles          INTEGER,

    notes                   TEXT,
    metadata                JSONB NOT NULL DEFAULT '{}',

    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Deferrable so a reorder can shuffle positions inside one transaction
    -- without colliding half-way through. (A deferrable constraint cannot be
    -- an ON CONFLICT target — nothing upserts on position, so that is fine.)
    CONSTRAINT journey_segments_position_key
        UNIQUE (journey_id, position) DEFERRABLE INITIALLY DEFERRED,

    -- Redundant as a uniqueness claim — `id` is already the primary key — but
    -- it is what lets the subtype tables below foreign-key `(id, mode)` and so
    -- be pinned to a segment of the matching mode.
    CONSTRAINT journey_segments_mode_key
        UNIQUE (id, mode),

    CONSTRAINT journey_segments_position_check
        CHECK (position >= 0),
    CONSTRAINT journey_segments_window_check
        CHECK (started_at IS NULL OR ended_at IS NULL OR ended_at >= started_at),
    CONSTRAINT journey_segments_duration_check
        CHECK (duration_minutes IS NULL OR duration_minutes >= 0),
    CONSTRAINT journey_segments_distance_check
        CHECK (distance_miles IS NULL OR distance_miles >= 0),
    CONSTRAINT journey_segments_endpoints_check
        CHECK (
            origin_place_id IS NULL
            OR destination_place_id IS NULL
            OR origin_place_id <> destination_place_id
        )
);

CREATE INDEX idx_journey_segments_journey_position
    ON journey_segments (journey_id, position);

CREATE INDEX idx_journey_segments_mode
    ON journey_segments (mode);

CREATE INDEX idx_journey_segments_started
    ON journey_segments (started_at);

CREATE INDEX idx_journey_segments_origin
    ON journey_segments (origin_place_id);

CREATE INDEX idx_journey_segments_destination
    ON journey_segments (destination_place_id);

-- ---------------------------------------------------------------------------
-- segment_flights — a flight leg
-- ---------------------------------------------------------------------------
-- A flight is a shared, real-world thing: AA100 on a given day is one row in
-- `flights` however many users were aboard. So this table holds the reference
-- plus only what is personal to this traveller. Flight number, schedule, and
-- distance all live on `flights`, where POST /api/v1/flights already puts them.

CREATE TABLE segment_flights (
    segment_id      UUID PRIMARY KEY,

    -- Fixed to 'flight' so the composite foreign key can only match a segment
    -- whose mode is 'flight'.
    mode            VARCHAR(20) NOT NULL DEFAULT 'flight'
        CHECK (mode = 'flight'),

    flight_id       UUID NOT NULL REFERENCES flights(id),

    seat            VARCHAR(10),
    cabin           VARCHAR(20)
        CHECK (cabin IS NULL OR cabin IN ('economy', 'premium_economy', 'business', 'first')),
    booking_reference VARCHAR(20),
    ticket_number   VARCHAR(20),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT segment_flights_segment_fkey
        FOREIGN KEY (segment_id, mode)
        REFERENCES journey_segments (id, mode)
        ON DELETE CASCADE
);

-- "Who else was on my flight" reads this way round.
CREATE INDEX idx_segment_flights_flight ON segment_flights (flight_id);

-- ---------------------------------------------------------------------------
-- segment_drives — a car leg
-- ---------------------------------------------------------------------------
-- Unlike a flight, a drive has no shared real-world counterpart, so its details
-- live here outright. Duration and distance are not repeated — they are on the
-- parent, the same as for every other mode.

CREATE TABLE segment_drives (
    segment_id      UUID PRIMARY KEY,

    mode            VARCHAR(20) NOT NULL DEFAULT 'drive'
        CHECK (mode = 'drive'),

    vehicle_id      BIGINT REFERENCES vehicles(id) ON DELETE SET NULL,
    role            VARCHAR(20)
        CHECK (role IS NULL OR role IN ('driver', 'passenger')),

    -- Encoded path, for drawing the leg on the globe rather than a straight line.
    route_polyline  TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT segment_drives_segment_fkey
        FOREIGN KEY (segment_id, mode)
        REFERENCES journey_segments (id, mode)
        ON DELETE CASCADE
);

CREATE INDEX idx_segment_drives_vehicle ON segment_drives (vehicle_id);

-- ---------------------------------------------------------------------------
-- journey_totals — the roll-up, mode-agnostic
-- ---------------------------------------------------------------------------
-- The payoff of putting distance and duration on the parent: one query covers
-- flights, drives, and everything added later.

CREATE VIEW journey_totals AS
SELECT
    j.id                                AS journey_id,
    COUNT(s.id)                         AS segment_count,
    COUNT(DISTINCT s.mode)              AS mode_count,
    COALESCE(SUM(s.distance_miles), 0)  AS total_distance_miles,
    COALESCE(SUM(s.duration_minutes), 0) AS total_duration_minutes,
    MIN(s.started_at)                   AS first_departure_at,
    MAX(s.ended_at)                     AS last_arrival_at
FROM journeys j
LEFT JOIN journey_segments s ON s.journey_id = j.id
GROUP BY j.id;

-- ---------------------------------------------------------------------------
-- updated_at triggers, matching the tables from 0002/0003
-- ---------------------------------------------------------------------------
-- `set_updated_at()` comes from 0001_create_users.sql.

DROP TRIGGER IF EXISTS places_set_updated_at ON places;
CREATE TRIGGER places_set_updated_at
BEFORE UPDATE ON places
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS vehicles_set_updated_at ON vehicles;
CREATE TRIGGER vehicles_set_updated_at
BEFORE UPDATE ON vehicles
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS journeys_set_updated_at ON journeys;
CREATE TRIGGER journeys_set_updated_at
BEFORE UPDATE ON journeys
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS journey_segments_set_updated_at ON journey_segments;
CREATE TRIGGER journey_segments_set_updated_at
BEFORE UPDATE ON journey_segments
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS segment_flights_set_updated_at ON segment_flights;
CREATE TRIGGER segment_flights_set_updated_at
BEFORE UPDATE ON segment_flights
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS segment_drives_set_updated_at ON segment_drives;
CREATE TRIGGER segment_drives_set_updated_at
BEFORE UPDATE ON segment_drives
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

COMMIT;

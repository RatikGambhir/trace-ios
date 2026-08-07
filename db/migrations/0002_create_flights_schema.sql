-- 0002_create_flights_schema.sql
-- Airports, airlines, and flights.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE airports (
    id              BIGSERIAL PRIMARY KEY,
    iata_code       CHAR(3) NOT NULL UNIQUE,
    icao_code       CHAR(4) UNIQUE,
    name            VARCHAR(150) NOT NULL,
    city            VARCHAR(100),
    country_code    CHAR(2),
    latitude        DECIMAL(9, 6),
    longitude       DECIMAL(9, 6),
    timezone        VARCHAR(50),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE airlines (
    id              BIGSERIAL PRIMARY KEY,
    iata_code       CHAR(2) UNIQUE,
    icao_code       CHAR(3) UNIQUE,
    name            VARCHAR(150) NOT NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE flights (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    airline_id              BIGINT NOT NULL
                            REFERENCES airlines(id),

    flight_number           VARCHAR(10) NOT NULL,

    origin_airport_id       BIGINT NOT NULL
                            REFERENCES airports(id),

    destination_airport_id  BIGINT NOT NULL
                            REFERENCES airports(id),

    distance_miles          INTEGER,

    scheduled_departure_at  TIMESTAMPTZ NOT NULL,
    scheduled_arrival_at    TIMESTAMPTZ NOT NULL,

    actual_departure_at     TIMESTAMPTZ,
    actual_arrival_at       TIMESTAMPTZ,

    status                  VARCHAR(20) NOT NULL DEFAULT 'scheduled'
        CHECK (
            status IN (
                'scheduled',
                'boarding',
                'departed',
                'in_air',
                'landed',
                'delayed',
                'cancelled',
                'diverted'
            )
        ),

    departure_terminal      VARCHAR(10),
    departure_gate          VARCHAR(10),

    arrival_terminal        VARCHAR(10),
    arrival_gate            VARCHAR(10),

    aircraft_type           VARCHAR(50),
    aircraft_registration   VARCHAR(20),

    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT flights_origin_destination_check
        CHECK (origin_airport_id <> destination_airport_id),

    CONSTRAINT flights_distance_check
        CHECK (distance_miles IS NULL OR distance_miles >= 0),

    CONSTRAINT flights_schedule_check
        CHECK (scheduled_arrival_at > scheduled_departure_at),

    CONSTRAINT flights_unique_instance
        UNIQUE (
            airline_id,
            flight_number,
            scheduled_departure_at
        )
);

CREATE INDEX idx_flights_origin_departure
    ON flights (
        origin_airport_id,
        scheduled_departure_at
    );

CREATE INDEX idx_flights_destination_arrival
    ON flights (
        destination_airport_id,
        scheduled_arrival_at
    );

CREATE INDEX idx_flights_airline_number
    ON flights (
        airline_id,
        flight_number
    );

CREATE INDEX idx_flights_status
    ON flights (status);

CREATE INDEX idx_flights_scheduled_departure
    ON flights (scheduled_departure_at);

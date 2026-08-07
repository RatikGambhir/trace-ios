-- 0005_add_journey_idempotency.sql
-- Idempotency support for POST /api/v1/journeys.
--
-- A flight has a natural key — (airline, flight_number, scheduled_departure_at)
-- — so replaying its creation is unambiguous. A journey has no such key: two
-- trips with the same title and dates can both be real. So idempotency here is
-- opt-in, on a key the client chooses.
--
-- Transactional for the same reason as 0004: whole, or not at all.

BEGIN;

ALTER TABLE journeys
    ADD COLUMN idempotency_key VARCHAR(64);

-- Partial, so the many journeys created without a key do not collide with each
-- other. Scoped to the user: one client's key cannot collide with another's.
CREATE UNIQUE INDEX journeys_user_idempotency_key
    ON journeys (user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- Lets a drive segment name a vehicle by nickname and get the same row back
-- every time, instead of a new "The Subaru" per journey.
CREATE UNIQUE INDEX vehicles_user_nickname_key
    ON vehicles (user_id, nickname)
    WHERE nickname IS NOT NULL;

COMMIT;

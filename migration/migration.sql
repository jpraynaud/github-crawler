CREATE SCHEMA IF NOT EXISTS github;
CREATE TABLE IF NOT EXISTS github.repository (
    id SERIAL PRIMARY KEY,
    repository_name TEXT NOT NULL,
    organization_name TEXT NOT NULL,
    total_stars INT NOT NULL,
    UNIQUE (repository_name, organization_name)
);
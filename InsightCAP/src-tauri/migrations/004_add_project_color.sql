-- migration: 004_add_project_color.sql
-- Add color column to projects table for group color customization

ALTER TABLE projects ADD COLUMN color TEXT;

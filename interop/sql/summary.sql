-- Aggregate actual case results; a failed language is never hidden by an overall average.
SELECT language, COUNT(*) AS cases, SUM(passed) AS passed,
       SUM(CASE WHEN passed = 0 THEN 1 ELSE 0 END) AS failed
FROM conformance
GROUP BY language
ORDER BY language;

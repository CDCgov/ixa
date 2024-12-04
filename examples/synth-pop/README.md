# Synthetic population querying

This example does three basic things:
1) Reads a synthetic population and creates people with the variables from the data base
2) Queries people in the population based on their geographical location and age group
3) Distributes vaccines for valid age groups/geographical location

## Geographical locations
Location IDs are generated from census tract FIPS code (11 digits 2 State + 3 County + 6 census tract) followed by locaiton ID within that tract (number of digits determined by location type, e.g. home IDs have 4, schools 3, work 5)

## Age groups

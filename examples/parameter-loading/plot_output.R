library(tidyverse)
library(jsonlite)
dir <- file.path("examples", "parameter-loading")
params <- read_json(file.path(dir, "input.json"))
population <- params$population
foi <- params$foi

output_df <- read_csv(file.path(dir, "incidence.csv")) |>
  dplyr::filter(infection_status == "I") |>
  group_by(time) |>
  mutate(inf = n()) |>
  ungroup() |>
  mutate(inf = cumsum(inf))

time_array <- 0:ceiling(max(output_df$time))

expected_susc <- population * exp(-foi * time_array)

plot(output_df$time, population - output_df$inf, ylim = c(0,population))
lines(time_array, expected_susc, col = "red")

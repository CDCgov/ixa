library(tidyverse)
library(jsonlite)
## Todo:
## - Plot population changes
## - Plot SIR
## - Compare with theoretical foi with population change
dir <- file.path("examples", "births-deaths")
params <- read_json(file.path(dir, "input.json"))
population <- params$population
age_groups <- params$foi_groups



output_df <- read_csv(file.path(dir, "incidence.csv")) |>
  dplyr::filter(infection_status == "I") |>
  group_by(time, age_group) |>
  mutate(inf = n()) |>
  ungroup() |>
  mutate(inf = cumsum(inf))

time_array <- 0:ceiling(max(output_df$time))

layout(matrix(seq_along(age_groups), nrow = length(age_groups)))
for (a in seq_along(age_groups)) {
  foi <- age_groups[[a]]$foi
  age_group_name <- age_groups[[a]]$group_name
  expected_susc <- population * exp(-foi * time_array)
  tmp_df <- filter(output_df, age_group == age_group_name)
  plot(tmp_df$time, tmp_df$inf, ylim = c(0,population), main = age_group_name)
  lines(time_array, expected_susc, col = "red")
}

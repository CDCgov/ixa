library(tidyverse)

population = 1000
foi = 0.1
output_df <- read_csv("./examples/time-varying-infection/incidence.csv") |>
    filter(infection_status == "I") |>
    group_by(time) |>
    mutate(inf = n()) |>
    ungroup() |>
    mutate(inf = cumsum(inf))

time_array = 0:ceiling(max(output_df$time))

expected_susc = population * exp(-foi * time_array)

plot(output_df$time, population - output_df$inf, ylim = c(0,population))
lines(time_array, expected_susc, col = "red")

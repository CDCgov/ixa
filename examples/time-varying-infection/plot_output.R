library(readr)
library(dplyr)
library(purrr)

population <- 1000
foi <- 0.15
foi_sin_shift <- 3
output_df <- readr::read_csv("./examples/time-varying-infection/incidence.csv") |>
    dplyr::filter(infection_status == "I") |>
    dplyr::group_by(time) |>
    dplyr::mutate(inf = n()) |>
    dplyr::ungroup() |>
    dplyr::mutate(inf = cumsum(inf))

time_array <- 0:ceiling(max(output_df$time))

# dS / dt = -foi(t) * S(t) # nolint: commented_code_linter.
foi_t <- function(t) {
  return(foi * (sin(t + foi_sin_shift) + 1))
}

expected_susc <- purrr::map(time_array,
function(x) {population * exp(-integrate(foi_t,
lower = 0, upper = x)$value)}) |>
    unlist()

plot(output_df$time, population - output_df$inf, ylim = c(0, population))
lines(time_array, expected_susc, col = "red")

library(readr)
library(dplyr)
library(purrr)
library(ggplot2)
library(jsonlite)

parameters <- jsonlite::read_json(file.path("examples",
"time-varying-infection",
"input.json"))

output_df <- readr::read_csv(file.path("examples",
"time-varying-infection",
"incidence.csv")) |>
    dplyr::filter(infection_status == "I") |>
    dplyr::group_by(time) |>
    dplyr::summarise(inf = n()) |>
    dplyr::ungroup() |>
    dplyr::mutate(inf = cumsum(inf))

time_array <- seq(0, ceiling(max(output_df$time)), 0.1)

# dS / dt = -foi(t) * S(t) # nolint: commented_code_linter.
foi_t <- function(t) {
  return(parameters$foi * (sin(t + parameters$foi_sin_shift) + 1))
}

expected_susc <- purrr::map(time_array,
function(x) {parameters$population * exp(-integrate(foi_t,
lower = 0, upper = x)$value)}) |>
    unlist()

ggplot2::ggplot() +
geom_point(aes(output_df$time, parameters$population - output_df$inf)) +
geom_line(aes(time_array, expected_susc), color = "red") +
xlab("Time") +
ylab("Susceptibles")

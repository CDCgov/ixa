library(readr)
library(dplyr)
library(purrr)
library(ggplot2)
library(jsonlite)

parameters <- jsonlite::read_json(file.path(
  "examples",
  "time-varying-infection",
  "input.json"
))

output_df <- readr::read_csv(file.path(
  "examples",
  "time-varying-infection",
  "incidence.csv"
)) |>
  dplyr::group_by(infection_status, time) |>
  dplyr::summarise(count = n()) |>
  dplyr::ungroup(time) |>
  # want to calculate number of remaining susceptibles from
  # population minus number of cumulative infections -- because
  # everyone gets infected
  dplyr::mutate(count = dplyr::case_when(
    infection_status == "I" ~ parameters$population - cumsum(count),
    infection_status == "R" ~ cumsum(count)
  )) |>
  # but now infection_status "I" really tells us the number of susceptible people
  dplyr::mutate(infection_status = dplyr::case_when(
    infection_status == "I" ~ "S",
    TRUE ~ infection_status
  ))

time_array_susc <- seq(0, ceiling(max(n_inf_output_df$time)), 0.1)

# dS / dt = -foi(t) * S(t) # nolint: commented_code_linter.
foi_t <- function(t) {
  return(parameters$foi * (sin(t + parameters$foi_sin_shift) + 1))
}

expected_susc <- purrr::map(
  time_array_susc,
  function(x) {
    parameters$population * exp(-integrate(foi_t,
      lower = 0, upper = x
    )$value)
  }
) |>
  unlist()

ggplot2::ggplot() +
  geom_point(aes(time, count, color = infection_status), output_df) +
  geom_line(aes(time_array_susc, expected_susc), color = "black") +
  xlab("Time") +
  ylab("People") +
  scale_y_log10()

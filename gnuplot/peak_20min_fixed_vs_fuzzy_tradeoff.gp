data_dir = "tests/simulation_results"
output_file = data_dir . "/peak_20min_fixed_vs_fuzzy_tradeoff.png"

set terminal pngcairo size 1400,1000 enhanced font "Arial,14"
set output output_file
set datafile separator comma
set grid
set key outside right top

stats data_dir . "/fuzzy_peak_20min.csv" using 7 nooutput
fuzzy_ped = STATS_mean
stats data_dir . "/fuzzy_peak_20min.csv" using 10 nooutput
fuzzy_veh = STATS_mean

set title "Fixed-Time Tradeoff Cloud vs Fuzzy Control"
set xlabel "Average Vehicle Queue Length"
set ylabel "Average Pedestrian Queue Length"

set label 1 sprintf("Fuzzy avg queues\nPed = %.2f\nVeh = %.2f", fuzzy_ped, fuzzy_veh) \
    at graph 0.72, graph 0.92 \
    front left \
    tc rgb "#222222"

# plot \
#     data_dir . "/fixed_peak_20min_surface.csv" using 13:12 with points pt 7 ps 0.6 lc rgb "#9aa0a6" title "Fixed-time combinations", \
#     '+' using (fuzzy_veh):(fuzzy_ped) with points pt 5 ps 3 lc rgb "#d62728" title "Fuzzy control"

# plot with purple
plot \
    data_dir . "/fixed_peak_20min_surface.csv" using 13:12 with points pt 7 ps 0.8 lc rgb "#f3a9ff" title "Fixed-time combinations", \
    '+' using (fuzzy_veh):(fuzzy_ped) with points pt 7 ps 3 lc rgb "#cb59ff" title "Fuzzy control"
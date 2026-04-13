data_dir = "tests/simulation_results"
output_file = data_dir . "/peak_20min_ped_queue_surface.png"

set terminal pngcairo size 1800,1200 enhanced font "Arial,14"
set output output_file

set title "20-Minute Peak Average Pedestrian Queue Length Surface"
set xlabel "Vehicle Green Service Time (s)"
set ylabel "Pedestrian Green Service Time (s)"
set zlabel "Average Pedestrian Queue Length"
set grid
set hidden3d
set ticslevel 0
set view 60,35,1.1,1.0
set dgrid3d 46,46 qnorm 2
set datafile separator comma

splot data_dir . "/fixed_peak_20min_surface.csv" using 3:2:12 with lines lc rgb "#1f77b4" title "Avg Pedestrian Queue"

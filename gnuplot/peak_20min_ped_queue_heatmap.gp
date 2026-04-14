data_dir = "tests/simulation_results"
output_file = data_dir . "/peak_20min_ped_queue_heatmap.png"

set terminal pngcairo size 1600,1200 enhanced font "Arial,14"
set output output_file

set title "20-Minute Peak Average Pedestrian Queue Length Heatmap"
set xlabel "Vehicle Green Service Time (s)"
set ylabel "Pedestrian Green Service Time (s)"
set cblabel "Average Pedestrian Queue Length"
set grid xtics ytics
set datafile separator comma

set xrange [15:60]
set yrange [15:60]
set xtics 15,5,60
set ytics 15,5,60
set view map
set size ratio -1
set dgrid3d 46,46 qnorm 2
set pm3d map
set palette rgbformulae 33,13,10

splot data_dir . "/fixed_peak_20min_surface.csv" using 3:2:12 with pm3d notitle

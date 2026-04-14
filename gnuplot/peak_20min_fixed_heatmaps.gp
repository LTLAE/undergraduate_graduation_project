data_dir = "tests/simulation_results"
output_file = data_dir . "/peak_20min_fixed_heatmaps.png"

set terminal pngcairo size 1900,900 enhanced font "Arial,14"
set output output_file
set datafile separator comma

set multiplot layout 1,2 title "Fixed-Time Control Under 20-Minute Peak: Queue Heatmaps"

set view map
set size ratio -1
set xrange [15:60]
set yrange [15:60]
set xlabel "Vehicle Green Service Time (s)"
set ylabel "Pedestrian Green Service Time (s)"
set xtics 15,5,60
set ytics 15,5,60
set grid front

set palette defined (0 '#5BCEFA', 0.5 '#FFFFFF', 1 '#F5A9B8')     # pink-white-blue
# set palette defined (0 '#2C2C2C', 0.33 '#9C59D1', 0.66 '#FFFFFF', 1 '#FCF434')      # yellow-white-purple-black
# but the fuck someone told me this look likes Kobe Bryant
# ABSOLUTELYTRUEDUDE -- xqc

set title "Average Pedestrian Queue Length"
set cblabel "Pedestrian Queue"
plot data_dir . "/fixed_peak_20min_surface.csv" using 3:2:12 with image notitle

set title "Average Vehicle Queue Length"
set cblabel "Vehicle Queue"
plot data_dir . "/fixed_peak_20min_surface.csv" using 3:2:13 with image notitle

unset multiplot

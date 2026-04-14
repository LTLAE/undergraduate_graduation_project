data_dir = "tests/simulation_results"
output_file = data_dir . "/peak_20min_ped_queue_contour.png"

set terminal pngcairo size 1600,1200 enhanced font "Arial,14"
set output output_file

set title "20-Minute Peak Average Pedestrian Queue Length Contour"
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
set contour base
set cntrparam levels incremental 0,20,400
unset surface
set table $contours
    splot data_dir . "/fixed_peak_20min_surface.csv" using 3:2:12
unset table

set pm3d map
set palette rgbformulae 33,13,10
splot data_dir . "/fixed_peak_20min_surface.csv" using 3:2:12 with pm3d notitle, \
      $contours using 1:2:3 with lines lw 1.5 lc rgb "#202020" title "Contour"

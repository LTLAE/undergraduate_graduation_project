data_dir = "tests/simulation_results"
output_file = data_dir . "/peak_20min_queue_lines.png"

set terminal pngcairo size 1800,1000 enhanced font "Arial,14"
set output output_file

set title "20-Minute Peak Queue Length Comparison"
set xlabel "Time (minutes)"
set ylabel "Queue Length"
set y2label "Pedestrian Green Extension (seconds)"
set y2tics
set grid xtics ytics
set key outside right top
set datafile separator comma
set yrange [-5:*]

set xrange [0:20]
set xtics 0,2,20

plot \
    data_dir . "/fixed_peak_20min.csv" using ($1/60.0):7 with lines lw 2 lc rgb "#d62728" title "Fixed Pedestrian Queue", \
    data_dir . "/fixed_peak_20min.csv" using ($1/60.0):10 with lines lw 2 lc rgb "#ff9896" title "Fixed Vehicle Queue", \
    data_dir . "/fuzzy_peak_20min.csv" using ($1/60.0):10 with lines lw 2 lc rgb "#9ecae1" title "Fuzzy Vehicle Queue", \
    data_dir . "/fuzzy_peak_20min.csv" using ($1/60.0):11 axes x1y2 with lines lw 2 dt 2 lc rgb "#2ca02c" title "Pedestrian Green Extension", \
    data_dir . "/fuzzy_peak_20min.csv" using ($1/60.0):7 with lines lw 3 lc rgb "#1f77b4" title "Fuzzy Pedestrian Queue"

/// <reference path="d3.d.ts" />

interface Span {
    start: number;
    end: number;
    name: string;
    children: SpanOrNote[];
    depth?: number;
}

interface Note {
    instant: number;
    name: string;
    description?: string;
    depth?: number;
}

function is_note(son: SpanOrNote): son is Note {
    let s = son as any;
    return typeof s.instant === "number" &&
           typeof s.name === "string";
}

function is_span(son: SpanOrNote): son is Span {
    let s = son as any;
    return typeof s.start === "number" &&
           typeof s.end   === "number" &&
           typeof s.name  === "string";
}

type SpanOrNote = Span | Note;

declare var data: SpanOrNote[];

let all_timestamps = [];

function linearize(span, depth, out, max_depth): [SpanOrNote[], number] {
    if (out === null || out === undefined) {
        out = [];
    }

    span.depth = depth;
    if (depth > max_depth) {
        max_depth = depth;
    }

    all_timestamps.push(span.start);
    all_timestamps.push(span.end);
    out.push(span);

    for (let i = 0; i < span.children.length; i++) {
        let [, d2] = linearize(span.children[i], depth + 1, out, max_depth);
        if (d2 > max_depth) {
            max_depth = d2;
        }
    }

    return [out, max_depth];
}

let [out, max_depth] = linearize(data, 0, null, 0);

let min_timestamp = all_timestamps.reduce(function (a, b) {return Math.min(a, b); });

let width = document.body.clientWidth;
let barHeight = 20;

let x = d3.scale.linear().domain(all_timestamps).range([0, width]);

let axis_lines_height = 5;
let axis_text_height = 50;

let axis_height = axis_lines_height + axis_text_height;
let chart = d3.select(".chart")
.attr("width", width)
.attr("height", barHeight * max_depth + axis_height);
let ease = "sine";
let duration = 300;
let axis =
    d3.svg.axis()
      .scale(x)
      .orient("bottom")
      .tickPadding(20)
      .tickSize(axis_lines_height)
      .tickFormat(function (n) {
          return "" + ((n - min_timestamp) / 1e6);
      });

function update(selector, data, scale) {
    console.log(selector, data, scale);

    axis.scale(scale);
    chart.select("#axis")
         .transition().duration(duration).ease(ease)
         .call(axis);

    let bar = chart.select(selector).selectAll("g").data(out);

    let group = bar.enter().append("g");
    group.append("rect");
    group.append("text");

    bar
       .transition().duration(duration).ease(ease)
       .attr("transform", function(d: SpanOrNote) {
            if (is_span(d)) {
                let x_offset = scale(d.start);
                let y_offset = d.depth * barHeight + axis_height;
                return "translate(" + x_offset + ", " + y_offset + ")";
            } else {
                throw "I haven't implemented notes yet";
            }
        }
    );

    function resize_graph(d) {
        let new_x = d3.scale.linear().domain([d.start, d.end]).range([0, width]);
        update(selector, out, new_x);
    }

    // Rectangle
    bar.select("rect")
       .on("click", resize_graph)
       .transition().duration(duration).ease(ease)
       .attr("width", function(item) {
           if (is_span(item)) {
               return scale(item.end) - scale(item.start);
           } else {
               throw "I haven't implemented notes yet";
           }
        })
       .attr("height", barHeight - 1);

    // Text
    bar.select("text")
       .text(function(d) { return d.name; })
       .on("click", resize_graph)
       .transition().duration(duration).ease(ease)
       .attr("x", function(d) {
           if (is_span(d)) {
               let my_start = d.start;
               let my_end = d.end;
               let start_at_left = scale.invert(0);
               let start_at_right = scale.invert(width);

               if (my_start <= start_at_left && my_end >= start_at_right) {
                   return 5 + scale(start_at_left) - scale(my_start);
               }
           } else {
               throw "I haven't implemented notes yet";
           }

           return 5;
       })
       .attr("y", function(d) { return barHeight / 2; })
       .attr("dy", ".35em")
       .attr("width", function(d) {
           if (is_span(d)) {
               return scale(d.end) - scale(d.start);
           } else {
               throw "I haven't implemented notes yet";
           }
       });
}

update("#bars", out, x);

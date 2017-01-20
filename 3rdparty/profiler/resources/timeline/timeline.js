/// <reference path="d3.d.ts" />
function is_note(son) {
    var s = son;
    return typeof s.instant === "number" &&
        typeof s.name === "string";
}
function is_span(son) {
    var s = son;
    return typeof s.start === "number" &&
        typeof s.end === "number" &&
        typeof s.name === "string";
}
var all_timestamps = [];
function linearize(span, depth, out, max_depth) {
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
    for (var i = 0; i < span.children.length; i++) {
        var _a = linearize(span.children[i], depth + 1, out, max_depth), d2 = _a[1];
        if (d2 > max_depth) {
            max_depth = d2;
        }
    }
    return [out, max_depth];
}
var _a = linearize(data, 0, null, 0), out = _a[0], max_depth = _a[1];
var min_timestamp = all_timestamps.reduce(function (a, b) { return Math.min(a, b); });
var width = document.body.clientWidth;
var barHeight = 20;
var x = d3.scale.linear().domain(all_timestamps).range([0, width]);
var axis_lines_height = 5;
var axis_text_height = 50;
var axis_height = axis_lines_height + axis_text_height;
var chart = d3.select(".chart")
    .attr("width", width)
    .attr("height", barHeight * max_depth + axis_height);
var ease = "sine";
var duration = 300;
var axis = d3.svg.axis()
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
    var bar = chart.select(selector).selectAll("g").data(out);
    var group = bar.enter().append("g");
    group.append("rect");
    group.append("text");
    bar
        .transition().duration(duration).ease(ease)
        .attr("transform", function (d) {
        if (is_span(d)) {
            var x_offset = scale(d.start);
            var y_offset = d.depth * barHeight + axis_height;
            return "translate(" + x_offset + ", " + y_offset + ")";
        }
        else {
            throw "I haven't implemented notes yet";
        }
    });
    function resize_graph(d) {
        var new_x = d3.scale.linear().domain([d.start, d.end]).range([0, width]);
        update(selector, out, new_x);
    }
    // Rectangle
    bar.select("rect")
        .on("click", resize_graph)
        .transition().duration(duration).ease(ease)
        .attr("width", function (item) {
        if (is_span(item)) {
            return scale(item.end) - scale(item.start);
        }
        else {
            throw "I haven't implemented notes yet";
        }
    })
        .attr("height", barHeight - 1);
    // Text
    bar.select("text")
        .text(function (d) { return d.name; })
        .on("click", resize_graph)
        .transition().duration(duration).ease(ease)
        .attr("x", function (d) {
        if (is_span(d)) {
            var my_start = d.start;
            var my_end = d.end;
            var start_at_left = scale.invert(0);
            var start_at_right = scale.invert(width);
            if (my_start <= start_at_left && my_end >= start_at_right) {
                return 5 + scale(start_at_left) - scale(my_start);
            }
        }
        else {
            throw "I haven't implemented notes yet";
        }
        return 5;
    })
        .attr("y", function (d) { return barHeight / 2; })
        .attr("dy", ".35em")
        .attr("width", function (d) {
        if (is_span(d)) {
            return scale(d.end) - scale(d.start);
        }
        else {
            throw "I haven't implemented notes yet";
        }
    });
}
update("#bars", out, x);

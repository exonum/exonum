var TimespanSet = (function () {
    function TimespanSet(ts) {
        this.timespans = new Array();
        if (!(ts == null)) {
            this.timespans.push(ts);
        }
        this.startTime = ts.startTime;
        this.endTime = ts.endTime;
    }
    TimespanSet.prototype.add = function (t) {
        if (t.startTime < this.startTime) {
            this.startTime = t.startTime;
        }
        if (t.endTime > this.endTime) {
            this.endTime = t.endTime;
        }
        this.timespans.push(t);
    };
    return TimespanSet;
}());
function is_concurrent(ts1, ts2) {
    return (ts1.endTime > ts2.startTime && ts1.startTime < ts2.startTime) ||
        (ts2.endTime > ts1.startTime && ts2.startTime < ts1.endTime);
}
function get_non_overlapping(timespans) {
    var timespans = timespans.sort(function (ts1, ts2) {
        return ts1.startTime - ts2.startTime;
    });
    var sets = new Array();
    for (var _i = 0, timespans_1 = timespans; _i < timespans_1.length; _i++) {
        var t = timespans_1[_i];
        var found = false;
        for (var _a = 0, sets_1 = sets; _a < sets_1.length; _a++) {
            var s = sets_1[_a];
            if (!(is_concurrent(t, s))) {
                s.add(t);
                found = true;
                break;
            }
        }
        if (!found) {
            sets.push(new TimespanSet(t));
        }
    }
    return sets;
}

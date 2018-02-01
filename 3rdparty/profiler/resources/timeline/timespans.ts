
interface Timespan {
    startTime: number;
    endTime: number;
}

class TimespanSet implements Timespan {
    timespans: Array<Timespan>;
    startTime: number;
    endTime: number;
    constructor(ts?: Timespan) {
        this.timespans = new Array<Timespan>();
        if (!(ts == null)) {
            this.timespans.push(ts);
        }
        this.startTime = ts.startTime;
        this.endTime = ts.endTime; 
    }

    add(t: Timespan) {
        if (t.startTime < this.startTime) {
            this.startTime = t.startTime;
        }
        if (t.endTime > this.endTime) {
            this.endTime = t.endTime;
        }
        this.timespans.push(t);
    }
}

function is_concurrent(ts1: Timespan, ts2: Timespan) {
    return (ts1.endTime > ts2.startTime && ts1.startTime < ts2.startTime) ||
        (ts2.endTime > ts1.startTime && ts2.startTime < ts1.endTime);
}

function get_non_overlapping(timespans: Array<Timespan>) {
    var timespans = timespans.sort(
        function (ts1, ts2) {
            return ts1.startTime - ts2.startTime;
    });
    var sets = new Array<TimespanSet>(); 
    for (var t of timespans) {
        var found = false;
        for (var s of sets) {
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

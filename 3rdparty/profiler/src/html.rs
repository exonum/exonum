use std::io::Write;
use std::io::Result as IoResult;
use super::{SpanPtr, Span, ThreadFrame};
use std::collections::BTreeMap;
use std::rc::Rc;


pub fn dump_html<W: Write>(out: &mut W, frame: &ThreadFrame) -> IoResult<()> {
    fn dump_span<W: Write>(out: &mut W, name:&'static str, span: &Span) -> IoResult<()>{
            try!(writeln!(out, "{{"));
            try!(writeln!(out, r#"name: "{}","#, name));
            try!(writeln!(out, "value: {},", span.value));
            try!(writeln!(out, "count: {},", span.count));            
            try!(writeln!(out, "children: ["));
            try!(dump_spans(out, &span.children));
            try!(writeln!(out, "],"));
            try!(writeln!(out, "}}"));
            try!(writeln!(out, ","));
            Ok(())
    }
    fn dump_spans<W: Write>(out: &mut W, spans:& BTreeMap<&'static str, Rc<SpanPtr>> ) -> IoResult<()> {
        for (name, span) in spans.iter() {
            let span = span.borrow();
            try!(dump_span(out, name, &span));
        }
        Ok(())
    }
    
    
    try!(write!(out, r#"
<!doctype html>
<html>
    <head>
        <style>
            html, body {{
                width: 100%;
                height: 100%;
                margin: 0;
                padding: 0;
            }}
            {}
        </style>
        <script>
            {}
            {}
            {}
        </script>
    </head>
    <body>
        <script>
            var width = document.body.offsetWidth;
            var height = document.body.offsetHeight - 100;
            var flamegraph =
                d3.flameGraph()
                  .width(width)
                  .height(height)
                  .tooltip(false)
                  .sort(function(a, b){{
                    if (a.dummy){{
                        return 1
                    }}
                    if (b.value < a.value) {{
                        return -1;
                    }} else if (a.value > b.value) {{
                        return -1;
                    }} else {{
                        return 0;
                    }}
                  }});
            d3.select("body").datum({{ children: [
"#, include_str!("../resources/flameGraph.css"), include_str!("../resources/d3.js"), include_str!("../resources/d3-tip.js"), include_str!("../resources/flameGraph.js")));
    let spans = frame.root();
    let mut root = spans.borrow().clone();
    root.value = frame.time_since_start();
    try!(dump_span(out, "Self", &root));

    try!(write!(out, r#"]}}).call(flamegraph);
         </script>
    </body>
</html>"#));

    Ok(())
}

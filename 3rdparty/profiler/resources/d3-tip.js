
(function(root,factory){if(typeof define==='function'&&define.amd){define(['d3'],factory)}else if(typeof module==='object'&&module.exports){module.exports=function(d3){d3.tip=factory(d3)
return d3.tip}}else{root.d3.tip=factory(root.d3)}}(this,function(d3){return function(){var direction=d3_tip_direction,offset=d3_tip_offset,html=d3_tip_html,node=initNode(),svg=null,point=null,target=null
function tip(vis){svg=getSVGNode(vis)
point=svg.createSVGPoint()
document.body.appendChild(node)}
tip.show=function(){var args=Array.prototype.slice.call(arguments)
if(args[args.length-1]instanceof SVGElement)target=args.pop()
var content=html.apply(this,args),poffset=offset.apply(this,args),dir=direction.apply(this,args),nodel=getNodeEl(),i=directions.length,coords,scrollTop=document.documentElement.scrollTop||document.body.scrollTop,scrollLeft=document.documentElement.scrollLeft||document.body.scrollLeft
nodel.html(content).style({opacity:1,'pointer-events':'all'})
while(i--)nodel.classed(directions[i],false)
coords=direction_callbacks.get(dir).apply(this)
nodel.classed(dir,true).style({top:(coords.top+poffset[0])+scrollTop+'px',left:(coords.left+poffset[1])+scrollLeft+'px'})
return tip}
tip.hide=function(){var nodel=getNodeEl()
nodel.style({opacity:0,'pointer-events':'none'})
return tip}
tip.attr=function(n,v){if(arguments.length<2&&typeof n==='string'){return getNodeEl().attr(n)}else{var args=Array.prototype.slice.call(arguments)
d3.selection.prototype.attr.apply(getNodeEl(),args)}
return tip}
tip.style=function(n,v){if(arguments.length<2&&typeof n==='string'){return getNodeEl().style(n)}else{var args=Array.prototype.slice.call(arguments)
d3.selection.prototype.style.apply(getNodeEl(),args)}
return tip}
tip.direction=function(v){if(!arguments.length)return direction
direction=v==null?v:d3.functor(v)
return tip}
tip.offset=function(v){if(!arguments.length)return offset
offset=v==null?v:d3.functor(v)
return tip}
tip.html=function(v){if(!arguments.length)return html
html=v==null?v:d3.functor(v)
return tip}
tip.destroy=function(){if(node){getNodeEl().remove();node=null;}
return tip;}
function d3_tip_direction(){return'n'}
function d3_tip_offset(){return[0,0]}
function d3_tip_html(){return' '}
var direction_callbacks=d3.map({n:direction_n,s:direction_s,e:direction_e,w:direction_w,nw:direction_nw,ne:direction_ne,sw:direction_sw,se:direction_se}),directions=direction_callbacks.keys()
function direction_n(){var bbox=getScreenBBox()
return{top:bbox.n.y-node.offsetHeight,left:bbox.n.x-node.offsetWidth/2}}
function direction_s(){var bbox=getScreenBBox()
return{top:bbox.s.y,left:bbox.s.x-node.offsetWidth/2}}
function direction_e(){var bbox=getScreenBBox()
return{top:bbox.e.y-node.offsetHeight/2,left:bbox.e.x}}
function direction_w(){var bbox=getScreenBBox()
return{top:bbox.w.y-node.offsetHeight/2,left:bbox.w.x-node.offsetWidth}}
function direction_nw(){var bbox=getScreenBBox()
return{top:bbox.nw.y-node.offsetHeight,left:bbox.nw.x-node.offsetWidth}}
function direction_ne(){var bbox=getScreenBBox()
return{top:bbox.ne.y-node.offsetHeight,left:bbox.ne.x}}
function direction_sw(){var bbox=getScreenBBox()
return{top:bbox.sw.y,left:bbox.sw.x-node.offsetWidth}}
function direction_se(){var bbox=getScreenBBox()
return{top:bbox.se.y,left:bbox.e.x}}
function initNode(){var node=d3.select(document.createElement('div'))
node.style({position:'absolute',top:0,opacity:0,'pointer-events':'none','box-sizing':'border-box'})
return node.node()}
function getSVGNode(el){el=el.node()
if(el.tagName.toLowerCase()==='svg')
return el
return el.ownerSVGElement}
function getNodeEl(){if(node===null){node=initNode();document.body.appendChild(node);};return d3.select(node);}
function getScreenBBox(){var targetel=target||d3.event.target;while('undefined'===typeof targetel.getScreenCTM&&'undefined'===targetel.parentNode){targetel=targetel.parentNode;}
var bbox={},matrix=targetel.getScreenCTM(),tbbox=targetel.getBBox(),width=tbbox.width,height=tbbox.height,x=tbbox.x,y=tbbox.y
point.x=x
point.y=y
bbox.nw=point.matrixTransform(matrix)
point.x+=width
bbox.ne=point.matrixTransform(matrix)
point.y+=height
bbox.se=point.matrixTransform(matrix)
point.x-=width
bbox.sw=point.matrixTransform(matrix)
point.y-=height/2
bbox.w=point.matrixTransform(matrix)
point.x+=width
bbox.e=point.matrixTransform(matrix)
point.x-=width/2
point.y-=height/2
bbox.n=point.matrixTransform(matrix)
point.y+=height
bbox.s=point.matrixTransform(matrix)
return bbox}
return tip};}));
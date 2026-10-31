struct Out { @builtin(position) pos:vec4<f32>, @location(0) uv:vec2<f32> }
@group(0) @binding(0) var image:texture_2d<f32>;
@group(0) @binding(1) var image_sampler:sampler;
@vertex fn vs(@location(0) pos:vec2<f32>,@location(1) uv:vec2<f32>)->Out { var o:Out;o.pos=vec4<f32>(pos,0.,1.);o.uv=uv;return o; }
fn linear(c:vec3<f32>)->vec3<f32>{return select(pow((c+0.055)/1.055,vec3<f32>(2.4)),c/12.92,c<=vec3<f32>(0.04045));}
@fragment fn fs(i:Out)->@location(0) vec4<f32> {
 let p=textureSample(image,image_sampler,i.uv);let rgb=p.rgb/max(p.a,0.00001);
 let grid=vec2<u32>(i.uv*vec2<f32>(textureDimensions(image))/12.0);
 let checker=select(0.65,0.9,(grid.x+grid.y)%2u==0u);
 return vec4<f32>(linear(rgb)*p.a+linear(vec3<f32>(checker))*(1.0-p.a),1.0);
}

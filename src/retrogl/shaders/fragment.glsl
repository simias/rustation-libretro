#version 330 core

smooth in vec3 frag_shading_color;

out vec4 frag_color;

void main(){
  frag_color = vec4(frag_shading_color, 1.);
}

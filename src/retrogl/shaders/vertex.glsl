#version 330 core

in ivec2 coords;
in uvec3 color;

out vec3 frag_shading_color;

void main() {
  float x = (float(coords.x) / 512.) - 1.;
  float y = 1. - (float(coords.y) / 256.);

  frag_shading_color = vec3(color) / 255.;

  gl_Position = vec4(x, y, 0., 1.);
}

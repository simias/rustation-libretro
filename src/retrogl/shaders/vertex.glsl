#version 330 core

in ivec2 position;
in uvec3 color;

// Drawing offset
uniform ivec2 offset;

out vec3 frag_shading_color;

void main() {
  ivec2 pos = position + offset;

  float x = (float(pos.x) / 512.) - 1.;
  float y = 1. - (float(pos.y) / 256.);

  frag_shading_color = vec3(color) / 255.;

  gl_Position = vec4(x, y, 0., 1.);
}

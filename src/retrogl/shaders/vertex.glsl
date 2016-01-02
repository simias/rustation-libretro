#version 330 core

in vec2 coords;

void main() {
  gl_Position = vec4(coords, -1., 1.);
}

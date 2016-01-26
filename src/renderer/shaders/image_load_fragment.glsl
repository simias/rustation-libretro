#version 330 core

uniform sampler2D fb_texture;

in vec2 frag_fb_coord;

out vec4 frag_color;

// Read a pixel in VRAM
vec4 vram_get_pixel(int x, int y) {
  return texelFetch(fb_texture, ivec2(x, y), 0);
}

void main() {
  frag_color = vram_get_pixel(int(frag_fb_coord.x), int(frag_fb_coord.y));
}

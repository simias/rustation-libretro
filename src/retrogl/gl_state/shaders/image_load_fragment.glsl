#version 330 core

uniform usampler2D fb_texture;

in vec2 frag_fb_coord;

out vec4 frag_color;

// Read a 16bpp pixel in VRAM
uint vram_get_pixel(int x, int y) {
  return texelFetch(fb_texture, ivec2(x, y), 0).r;
}

// Convert a 16bit RGBA 5551 color into a normalized float RGBA
// vector.
vec4 texel_to_color(uint texel) {
  uint a = texel >> 15U;
  uint b = (texel >> 10U) & 0x1fU;
  uint g = (texel >> 5U) & 0x1fU;
  uint r = texel & 0x1fU;

  uvec4 color = uvec4(r, g, b, a);

  return vec4(color) / vec4(31., 31., 31., 1.);
}

void main() {
  uint texel = vram_get_pixel(int(frag_fb_coord.x), int(frag_fb_coord.y));

  frag_color = texel_to_color(texel);
}

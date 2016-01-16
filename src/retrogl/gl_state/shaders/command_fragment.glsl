#version 330 core

// Integer texture 16bit per pixel, stored in the red component
uniform usampler2D fb_texture;

in vec3 frag_shading_color;
// Texture page: base offset for texture lookup.
flat in uvec2 frag_texture_page;
// Texel coordinates within the page. Interpolated by OpenGL.
in vec2 frag_texture_coord;
// Clut coordinates in VRAM
flat in uvec2 frag_clut;
// 0: no texture, 1: raw-texture, 2: blended
flat in uint frag_texture_blend_mode;
// 0: 16bpp (no clut), 1: 8bpp, 2: 4bpp
flat in uint frag_depth_shift;
// 0: No dithering, 1: dithering enabled
flat in uint frag_dither;

out vec4 frag_color;

const uint BLEND_MODE_NO_TEXTURE    = 0U;
const uint BLEND_MODE_RAW_TEXTURE   = 1U;
const uint BLEND_MODE_TEXTURE_BLEND = 2U;

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

// Texture color 0x0000 is special in the Playstation GPU, it denotes
// a fully transparent texel (even for opaque draw commands). If you
// want black you have to use an opaque draw command and use `0x8000`
// instead.
// bool is_transparent(vec4 texel) {
//   return rebuild_color(texel) == 0;
// }

// PlayStation dithering pattern. The offset is selected based on the
// pixel position in VRAM, by blocks of 4x4 pixels. The value is added
// to the 8bit color components before they're truncated to 5 bits.
const int dither_pattern[16] =
  int[16](-4,  0, -3,  1,
           2, -2,  3, -1,
          -3,  1, -4,  0,
           3, -1,  2, -2);

void main() {

  vec4 color;

  if (frag_texture_blend_mode == BLEND_MODE_NO_TEXTURE) {
    color = vec4(frag_shading_color, 0.);
  } else {
    // Look up texture

    // Number of texel per VRAM 16bit "pixel" for the current depth
    uint pix_per_hw = 1U << frag_depth_shift;

    // 8 and 4bpp textures contain several texels per 16bit VRAM
    // "pixel"
    float tex_x_float = frag_texture_coord.x / float(pix_per_hw);

    // Texture pages are limited to 256x256 pixels
    int tex_x = int(tex_x_float) & 0xff;
    int tex_y = int(frag_texture_coord.y) & 0xff;

    tex_x += int(frag_texture_page.x);
    tex_y += int(frag_texture_page.y);

    uint texel = vram_get_pixel(tex_x, tex_y);

    if (frag_depth_shift > 0U) {
      // 8 and 4bpp textures are paletted so we need to lookup the
      // real color in the CLUT

      // A little bitwise magic to get the index in the CLUT. 4bpp
      // textures have 4 texels per VRAM "pixel", 8bpp have 2. We need
      // to shift the current color to find the proper part of the
      // halfword and then mask away the rest.

      // Bits per pixel (4 or 8)
      uint bpp = 16U >> frag_depth_shift;

      // 0xf for 4bpp, 0xff for 8bpp
      uint mask = ((1U << bpp) - 1U);

      // 0...3 for 4bpp, 1...2 for 8bpp
      uint align = uint(fract(tex_x_float) * pix_per_hw);

      // 0, 4, 8 or 12 for 4bpp, 0 or 8 for 8bpp
      uint shift = (align * bpp);

      // Finally we have the index in the CLUT
      uint index = (texel >> shift) & mask;

      int clut_x = int(frag_clut.x + index);
      int clut_y = int(frag_clut.y);

      // Look up the real color for the texel in the CLUT
      texel = vram_get_pixel(clut_x, clut_y);
    }

    // texel color 0x0000 is always fully transparent (even for opaque
    // draw commands)
    if (texel == 0U) {
      // Fully transparent texel, discard
      discard;
    }

    // We're done messing with integers, convert to the normalized
    // floating point representation OpenGL understands.
    vec4 tcolor = texel_to_color(texel);

    if (frag_texture_blend_mode == BLEND_MODE_RAW_TEXTURE) {
      color = tcolor;
    } else /* BLEND_MODE_TEXTURE_BLEND */ {
      // Blend the texel with the shading color. `frag_shading_color`
      // is multiplied by two so that it can be used to darken or
      // lighten the texture as needed. The result of the
      // multiplication should be saturated to 1.0 (0xff) but I think
      // OpenGL will take care of that since the output buffer holds
      // integers. The alpha/mask bit bit is taken directly from the
      // texture however.
      color = vec4(frag_shading_color * 2. * tcolor.rgb, tcolor.a);
    }
  }

  // Dithering
  int x_dither = int(gl_FragCoord.x) & 3;
  int y_dither = int(gl_FragCoord.y) & 3;

  // The multiplication by `frag_dither` will result in
  // `dither_offset` being 0 if dithering is disabled
  int dither_offset =
    dither_pattern[y_dither * 4 + x_dither] * int(frag_dither);

  float dither = float(dither_offset) / 255.;

  frag_color = color + vec4(dither, dither, dither, 0.);
}

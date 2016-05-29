#version 330 core

uniform sampler2D fb_texture;

// Scaling to apply to the dither pattern
uniform uint dither_scaling;
// 0: Only draw opaque pixels, 1: only draw semi-transparent pixels
uniform uint draw_semi_transparent;
// Texture window X mask
uniform uint tex_x_mask;
// Texture window X OR value
uniform uint tex_x_or;
// Texture window Y mask
uniform uint tex_y_mask;
// Texture window Y OR value
uniform uint tex_y_or;

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
// 0: Opaque primitive, 1: semi-transparent primitive
flat in uint frag_semi_transparent;

out vec4 frag_color;

const uint BLEND_MODE_NO_TEXTURE    = 0U;
const uint BLEND_MODE_RAW_TEXTURE   = 1U;
const uint BLEND_MODE_TEXTURE_BLEND = 2U;

// Read a pixel in VRAM
vec4 vram_get_pixel(uint x, uint y) {
  return texelFetch(fb_texture, ivec2(x & 0x3ffU, y & 0x1ffU), 0);
}

// Take a normalized color and convert it into a 16bit 1555 ABGR
// integer in the format used internally by the Playstation GPU.
uint rebuild_psx_color(vec4 color) {
  uint a = uint(floor(color.a + 0.5));
  uint r = uint(floor(color.r * 31. + 0.5));
  uint g = uint(floor(color.g * 31. + 0.5));
  uint b = uint(floor(color.b * 31. + 0.5));

  return (a << 15) | (b << 10) | (g << 5) | r;
}

// Texture color 0x0000 is special in the Playstation GPU, it denotes
// a fully transparent texel (even for opaque draw commands). If you
// want black you have to use an opaque draw command and use `0x8000`
// instead.
bool is_transparent(vec4 texel) {
  return rebuild_psx_color(texel) == 0U;
}

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

    // Texture pages are limited to 256x256 pixels
    uint tex_x = uint(frag_texture_coord.x) & 0xffU;
    uint tex_y = uint(frag_texture_coord.y) & 0xffU;

    // Texture window adjustments
    tex_x = (tex_x & tex_x_mask) | tex_x_or;
    tex_y = (tex_y & tex_y_mask) | tex_y_or;

    // Adjust x coordinate based on the texel color depth.
    uint tex_x_pix = tex_x / pix_per_hw;

    tex_x_pix += frag_texture_page.x;
    tex_y     += frag_texture_page.y;

    vec4 texel = vram_get_pixel(tex_x_pix, tex_y);

    if (frag_depth_shift > 0U) {
      // 8 and 4bpp textures are paletted so we need to lookup the
      // real color in the CLUT

      uint icolor = rebuild_psx_color(texel);

      // A little bitwise magic to get the index in the CLUT. 4bpp
      // textures have 4 texels per VRAM "pixel", 8bpp have 2. We need
      // to shift the current color to find the proper part of the
      // halfword and then mask away the rest.

      // Bits per pixel (4 or 8)
      uint bpp = 16U >> frag_depth_shift;

      // 0xf for 4bpp, 0xff for 8bpp
      uint mask = ((1U << bpp) - 1U);

      // 0...3 for 4bpp, 0...1 for 8bpp
      uint align = tex_x & ((1U << frag_depth_shift) - 1U);

      // 0, 4, 8 or 12 for 4bpp, 0 or 8 for 8bpp
      uint shift = (align * bpp);

      // Finally we have the index in the CLUT
      uint index = (icolor >> shift) & mask;

      uint clut_x = frag_clut.x + index;
      uint clut_y = frag_clut.y;

      // Look up the real color for the texel in the CLUT
      texel = vram_get_pixel(clut_x, clut_y);
    }

    // texel color 0x0000 is always fully transparent (even for opaque
    // draw commands)
    if (is_transparent(texel)) {
      // Fully transparent texel, discard
      discard;
    }

    // Bit 15 (stored in the alpha) is used as a flag for
    // semi-transparency, but only if this is a semi-transparent draw
    // command
    uint transparency_flag = uint(floor(texel.a + 0.5));

    uint is_texel_semi_transparent = transparency_flag & frag_semi_transparent;

    if (is_texel_semi_transparent != draw_semi_transparent) {
      // We're not drawing those texels in this pass, discard
      discard;
    }

    if (frag_texture_blend_mode == BLEND_MODE_RAW_TEXTURE) {
      color = texel;
    } else /* BLEND_MODE_TEXTURE_BLEND */ {
      // Blend the texel with the shading color. `frag_shading_color`
      // is multiplied by two so that it can be used to darken or
      // lighten the texture as needed. The result of the
      // multiplication should be saturated to 1.0 (0xff) but I think
      // OpenGL will take care of that since the output buffer holds
      // integers. The alpha/mask bit bit is taken directly from the
      // texture however.
      color = vec4(frag_shading_color * 2. * texel.rgb, texel.a);
    }
  }

  // 4x4 dithering pattern scaled by `dither_scaling`
  uint x_dither = (uint(gl_FragCoord.x) / dither_scaling) & 3U;
  uint y_dither = (uint(gl_FragCoord.y) / dither_scaling) & 3U;

  // The multiplication by `frag_dither` will result in
  // `dither_offset` being 0 if dithering is disabled
  int dither_offset =
    dither_pattern[y_dither * 4U + x_dither] * int(frag_dither);

  float dither = float(dither_offset) / 255.;

  frag_color = color + vec4(dither, dither, dither, 0.);
}

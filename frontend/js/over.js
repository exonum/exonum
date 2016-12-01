$(function(){
	
	$('.portfolio a.over').each(function(){
		
		overlay = $('<span class="overlay"><span class="fui-eye"></span></span>');
		
		$(this).append( overlay );
		
	})
	
})